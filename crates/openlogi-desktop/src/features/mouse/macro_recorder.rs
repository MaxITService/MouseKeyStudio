//! Focus-scoped keyboard recording into the macro editor's existing text format.

use std::time::Instant;

use gpui::{App, Context, Keystroke, Modifiers, Subscription, Window};
use openlogi_core::binding::{KeyboardUsage, MacroStep};

use super::macro_editor::{MacroDraft, MacroPhase};
use super::view::MouseModelView;
use crate::features::binding_editor::Recording;

pub(super) struct MacroRecording {
    phase: MacroPhase,
    steps: Vec<MacroStep>,
    held: Vec<KeyboardUsage>,
    last_event: Option<Instant>,
    _subscriptions: Vec<Subscription>,
}

impl MacroRecording {
    fn edge(&mut self, step: MacroStep, now: Instant) {
        if let Some(last) = self.last_event {
            let millis =
                u64::try_from(now.saturating_duration_since(last).as_millis()).unwrap_or(u64::MAX);
            if millis > 0 {
                self.steps.push(MacroStep::Delay { millis });
            }
        }
        self.steps.push(step);
        self.last_event = Some(now);
    }

    fn press(&mut self, key: KeyboardUsage, now: Instant) {
        if !self.held.contains(&key) {
            self.held.push(key);
            self.edge(MacroStep::KeyDown(key), now);
        }
    }

    fn release(&mut self, key: KeyboardUsage, now: Instant) {
        if let Some(index) = self.held.iter().position(|held| *held == key) {
            self.held.remove(index);
            self.edge(MacroStep::KeyUp(key), now);
        }
    }

    fn finish(&mut self) {
        // A focus loss can hide the corresponding physical release. Balance
        // recorded presses so replay never leaves a key held unintentionally.
        for key in self.held.drain(..).rev() {
            self.steps.push(MacroStep::KeyUp(key));
        }
    }
}

pub(super) fn toggle_recording(
    draft: &mut MacroDraft,
    window: &mut Window,
    cx: &mut Context<MouseModelView>,
) {
    if draft.recording.is_some() {
        draft.stop_recording(window, cx);
        return;
    }
    let phase = draft.active_phase;
    let Ok(steps) = MacroStep::parse_sequence(&draft.phase_input(phase).read(cx).value()) else {
        draft.invalid = true;
        return;
    };
    let focus = draft.record_focus.clone();
    window.focus(&focus, cx);
    let target = window.window_handle();
    let view = cx.weak_entity();
    let interceptor = cx.intercept_keystrokes(move |event, window, cx| {
        if window.window_handle() != target {
            return;
        }
        if !focus.contains_focused(window, cx) {
            let _ = view.update(cx, |view, cx| {
                view.macro_draft.stop_recording(window, cx);
                cx.notify();
            });
            return;
        }
        cx.stop_propagation();
        let _ = view.update(cx, |view, cx| {
            let draft = &mut view.macro_draft;
            let held = window.modifiers();
            match Recording::from_press(&event.keystroke, held) {
                Recording::Cancelled => draft.stop_recording(window, cx),
                Recording::Chord(combo) => {
                    draft.record_modifiers(held, window, cx);
                    if let Some(recording) = &mut draft.recording {
                        recording.press(combo.key(), Instant::now());
                    }
                    draft.publish_recording(window, cx);
                }
                Recording::Unsupported | Recording::ShiftedSymbol => {
                    draft.stop_recording(window, cx);
                    draft.invalid = true;
                }
            }
            cx.notify();
        });
    });
    let focus_out = cx.on_focus_out(&draft.record_focus, window, |view, _, window, cx| {
        view.macro_draft.stop_recording(window, cx);
        cx.notify();
    });
    let activation = cx.observe_window_activation(window, |view, window, cx| {
        if !window.is_window_active() {
            view.macro_draft.stop_recording(window, cx);
            cx.notify();
        }
    });
    draft.recording = Some(MacroRecording {
        phase,
        steps,
        held: Vec::new(),
        last_event: None,
        _subscriptions: vec![interceptor, focus_out, activation],
    });
    draft.invalid = false;
}

impl MacroDraft {
    pub(super) fn stop_recording(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(mut recording) = self.recording.take() {
            recording.finish();
            self.phase_input(recording.phase).update(cx, |input, cx| {
                input.set_value(MacroStep::format_sequence(&recording.steps), window, cx);
            });
        }
    }

    fn publish_recording(&self, window: &mut Window, cx: &mut App) {
        if let Some(recording) = &self.recording {
            self.phase_input(recording.phase).update(cx, |input, cx| {
                input.set_value(MacroStep::format_sequence(&recording.steps), window, cx);
            });
        }
    }

    pub(super) fn record_key_up(&mut self, key: &Keystroke, window: &mut Window, cx: &mut App) {
        if let Some(recording) = &mut self.recording
            && let Recording::Chord(combo) = Recording::from(key)
        {
            recording.release(combo.key(), Instant::now());
            self.publish_recording(window, cx);
        }
    }

    pub(super) fn record_modifiers(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(recording) = &mut self.recording else {
            return;
        };
        let now = Instant::now();
        // GPUI reports modifier families, not physical left/right sides.
        for (code, pressed) in [
            (0xe0, modifiers.control),
            (0xe1, modifiers.shift),
            (0xe2, modifiers.alt),
            (0xe3, modifiers.platform),
        ] {
            let Ok(key) = KeyboardUsage::try_from(code) else {
                continue;
            };
            if pressed {
                recording.press(key, now);
            } else {
                recording.release(key, now);
            }
        }
        self.publish_recording(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn recording_preserves_edges_timing_and_balances_stop() {
        let now = Instant::now();
        let a: KeyboardUsage = "A".parse().unwrap();
        let ctrl: KeyboardUsage = "LeftCtrl".parse().unwrap();
        let mut recording = MacroRecording {
            phase: MacroPhase::OnPress,
            steps: Vec::new(),
            held: Vec::new(),
            last_event: None,
            _subscriptions: Vec::new(),
        };
        recording.press(ctrl, now);
        recording.press(a, now + Duration::from_millis(50));
        recording.press(a, now + Duration::from_millis(60)); // auto-repeat
        recording.release(a, now + Duration::from_millis(75));
        recording.finish();
        assert_eq!(
            recording.steps,
            [
                MacroStep::KeyDown(ctrl),
                MacroStep::Delay { millis: 50 },
                MacroStep::KeyDown(a),
                MacroStep::Delay { millis: 25 },
                MacroStep::KeyUp(a),
                MacroStep::KeyUp(ctrl)
            ]
        );
        let text = MacroStep::format_sequence(&recording.steps);
        assert_eq!(MacroStep::parse_sequence(&text).unwrap(), recording.steps);
    }
}
