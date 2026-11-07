//! Optional frame-paced wheel input, before dispatch to any component.
//! Keep line units: editors and lists choose their own line height.

use super::*;
use crate::ScrollDelta;

#[derive(Clone, Copy)]
struct WheelSmoothing(Option<Duration>);
impl Global for WheelSmoothing {}

impl App {
    /// Animate discrete mouse-wheel input in every window. Precise pixel input
    /// and reduced-motion input are dispatched directly. `None` disables it.
    pub fn set_scroll_wheel_smoothing(&mut self, duration: Option<Duration>) {
        self.set_global(WheelSmoothing(
            duration.filter(|duration| !duration.is_zero()),
        ));
    }
}

mod motion;
pub(super) use motion::Wheel;

impl Window {
    pub(super) fn queue_wheel(&mut self, event: &PlatformInput, cx: &mut App) -> bool {
        let duration = cx
            .try_global::<WheelSmoothing>()
            .and_then(|policy| policy.0);
        if let PlatformInput::ScrollWheel(wheel) = event
            && matches!(wheel.delta, ScrollDelta::Lines(_))
            && let Some(duration) = duration
            && !cx.reduce_motion()
        {
            self.mouse_position = wheel.position;
            self.modifiers = wheel.modifiers;
            self.wheel.push(wheel.clone(), duration);
            self.schedule_wheel_frame();
            return true;
        }
        // A hand resting on the mouse while turning the wheel jitters it by a
        // pixel or two; that is still the same scroll, so the tail continues
        // (the move itself is dispatched as usual for hover).
        if let PlatformInput::MouseMove(moved) = event
            && moved.pressed_button.is_none()
            && self
                .wheel
                .motion
                .as_ref()
                .is_some_and(|motion| motion.holds(moved.position, moved.modifiers))
        {
            return false;
        }
        // Clicking, navigating, dragging a scrollbar, changing modifiers or
        // moving away must not send a tail into the newly selected control.
        self.wheel.motion = None;
        false
    }

    /// Whether a wheel animation still needs frames.
    pub(super) fn wheel_in_motion(&self) -> bool {
        self.wheel.motion.is_some()
    }

    fn schedule_wheel_frame(&mut self) {
        if self.wheel_frame_pending || self.wheel.motion.is_none() {
            return;
        }
        self.wheel_frame_pending = true;
        self.on_next_frame(|window, cx| {
            window.wheel_frame_pending = false;
            if let Some(event) = window
                .wheel
                .advance(std::time::Instant::now(), cx.reduce_motion())
            {
                // Bypass the queue, retaining line semantics for each handler.
                window.dispatch_input(PlatformInput::ScrollWheel(event), cx);
            }
            window.schedule_wheel_frame();
        });
    }
}
