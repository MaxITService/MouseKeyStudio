//! Interruptible press/hold/release sequences, with RAII ownership of every key.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use openlogi_core::binding::{HeldKeys, KeyCombo, KeyboardUsage, Macro, MacroStep};

use super::{HeldChord, press_hold};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Signal {
    Release,
    Cancel,
}

/// One running sequence owned by its physical button's lifecycle.
/// Dropping it cancels pending delays and joins after releasing all output.
#[must_use]
pub struct MacroPlayback {
    control: Sender<Signal>,
    worker: Option<JoinHandle<()>>,
}

impl MacroPlayback {
    /// Start the release section without blocking the input worker on its delays.
    pub fn release(&self) {
        let _ = self.control.send(Signal::Release);
    }

    /// Whether playback (including its release section) has finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for MacroPlayback {
    fn drop(&mut self) {
        let _ = self.control.send(Signal::Cancel);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Run a macro off the input thread. Retain the returned owner until release
/// or cancellation; release sections can finish in a separate retained owner.
pub fn start_macro(sequence: &Macro) -> std::io::Result<MacroPlayback> {
    spawn_macro(sequence, true)
}

/// Play every phase once for a pulse or release-time action, retaining ownership
/// so cancellation and shutdown can still stop delays and balance output.
pub fn start_macro_once(sequence: &Macro) -> std::io::Result<MacroPlayback> {
    spawn_macro(sequence, false)
}

fn spawn_macro(sequence: &Macro, lifecycle: bool) -> std::io::Result<MacroPlayback> {
    let sequence = sequence.clone();
    let (control, receiver) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("openlogi-macro".into())
        .spawn(move || run(&sequence, &receiver, lifecycle, NativeKeys::default()))?;
    Ok(MacroPlayback {
        control,
        worker: Some(worker),
    })
}

/// A dispatcher without physical release context plays each section once and
/// balances the output, rather than leaving a held/repeating macro running.
pub(super) fn tap(sequence: &Macro) {
    let sequence = sequence.clone();
    let result = thread::Builder::new()
        .name("openlogi-macro-tap".into())
        .spawn(move || {
            let (_control, receiver) = mpsc::channel();
            run(&sequence, &receiver, false, NativeKeys::default());
        });
    if let Err(error) = result {
        tracing::warn!(%error, "could not start macro");
    }
}

trait KeySink {
    fn down(&mut self, key: KeyboardUsage);
    fn up(&mut self, key: KeyboardUsage);
}

#[derive(Default)]
struct NativeKeys {
    held: HashMap<KeyboardUsage, HeldChord>,
}

impl KeySink for NativeKeys {
    fn down(&mut self, key: KeyboardUsage) {
        self.held
            .entry(key)
            .or_insert_with(|| press_hold(&KeyCombo::unmodified(key), HeldKeys::WholeChord));
    }

    fn up(&mut self, key: KeyboardUsage) {
        self.held.remove(&key);
    }
}

struct OwnedKeys<S: KeySink> {
    sink: S,
    order: Vec<KeyboardUsage>,
}

impl<S: KeySink> OwnedKeys<S> {
    fn step(&mut self, step: &MacroStep) {
        match step {
            MacroStep::KeyDown(key) if !self.order.contains(key) => {
                // Record ownership before posting, so unwinding also cleans up.
                self.order.push(*key);
                self.sink.down(*key);
            }
            MacroStep::KeyUp(key) => {
                if let Some(index) = self.order.iter().position(|held| held == key) {
                    self.order.remove(index);
                    self.sink.up(*key);
                }
            }
            MacroStep::KeyDown(_) | MacroStep::Delay { .. } => {}
        }
    }
}

impl<S: KeySink> Drop for OwnedKeys<S> {
    fn drop(&mut self) {
        for key in self.order.drain(..).rev() {
            self.sink.up(key);
        }
    }
}

fn pending(receiver: &Receiver<Signal>, releasing: bool) -> Result<(), Signal> {
    loop {
        match receiver.try_recv() {
            Ok(Signal::Release) if releasing => {}
            Ok(signal) => return Err(signal),
            Err(TryRecvError::Disconnected) => return Err(Signal::Cancel),
            Err(TryRecvError::Empty) => return Ok(()),
        }
    }
}

fn wait(receiver: &Receiver<Signal>, duration: Duration, releasing: bool) -> Result<(), Signal> {
    let started = Instant::now();
    loop {
        let remaining = duration.saturating_sub(started.elapsed());
        match receiver.recv_timeout(remaining) {
            Ok(Signal::Release) if releasing => {}
            Ok(signal) => return Err(signal),
            Err(RecvTimeoutError::Disconnected) => return Err(Signal::Cancel),
            Err(RecvTimeoutError::Timeout) => return Ok(()),
        }
    }
}

fn steps<S: KeySink>(
    sequence: &[MacroStep],
    receiver: &Receiver<Signal>,
    keys: &mut OwnedKeys<S>,
    releasing: bool,
) -> Result<(), Signal> {
    for step in sequence {
        pending(receiver, releasing)?;
        if let MacroStep::Delay { millis } = step {
            wait(receiver, Duration::from_millis(*millis), releasing)?;
        } else {
            keys.step(step);
        }
    }
    Ok(())
}

fn run<S: KeySink>(sequence: &Macro, receiver: &Receiver<Signal>, lifecycle: bool, sink: S) {
    let mut keys = OwnedKeys {
        sink,
        order: Vec::new(),
    };
    let result = steps(&sequence.on_press, receiver, &mut keys, false).and_then(|()| {
        loop {
            steps(&sequence.while_held, receiver, &mut keys, false)?;
            if !lifecycle || !sequence.repeat_while_held || sequence.while_held.is_empty() {
                break;
            }
            // Preserve recorded timings; only guard loops with no timed steps.
            if !sequence
                .while_held
                .iter()
                .any(|step| matches!(step, MacroStep::Delay { millis } if *millis > 0))
            {
                wait(receiver, Duration::from_millis(10), false)?;
            }
        }
        if lifecycle && (!sequence.while_held.is_empty() || !sequence.on_release.is_empty()) {
            Err(receiver.recv().unwrap_or(Signal::Cancel))
        } else {
            Ok(())
        }
    });
    if result != Err(Signal::Cancel) {
        let _ = steps(&sequence.on_release, receiver, &mut keys, true);
    }
    // OwnedKeys balances incomplete and canceled sequences in reverse order.
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecordedKeys(Sender<(KeyboardUsage, bool, Instant)>);
    impl KeySink for RecordedKeys {
        fn down(&mut self, key: KeyboardUsage) {
            self.0.send((key, true, Instant::now())).unwrap();
        }
        fn up(&mut self, key: KeyboardUsage) {
            self.0.send((key, false, Instant::now())).unwrap();
        }
    }

    fn key(code: u8) -> KeyboardUsage {
        KeyboardUsage::try_new(code).unwrap()
    }

    fn held_sequence() -> Macro {
        Macro {
            name: "Right Ctrl + F8 Hold".into(),
            while_held: vec![
                MacroStep::KeyDown(key(0xe4)),
                MacroStep::Delay { millis: 50 },
                MacroStep::KeyDown(key(0x41)),
            ],
            on_release: vec![
                MacroStep::KeyUp(key(0x41)),
                MacroStep::Delay { millis: 50 },
                MacroStep::KeyUp(key(0xe4)),
            ],
            ..Macro::default()
        }
    }

    #[test]
    fn one_shot_runs_all_phases_once_even_when_repeat_is_enabled() {
        let (_control, receiver) = mpsc::channel();
        let (record, events) = mpsc::channel();
        let mut sequence = held_sequence();
        sequence.on_press = vec![MacroStep::KeyDown(key(4)), MacroStep::KeyUp(key(4))];
        sequence.repeat_while_held = true;
        run(&sequence, &receiver, false, RecordedKeys(record));
        let seen: Vec<_> = events.try_iter().map(|event| (event.0, event.1)).collect();
        assert_eq!(
            seen,
            [
                (key(4), true),
                (key(4), false),
                (key(0xe4), true),
                (key(0x41), true),
                (key(0x41), false),
                (key(0xe4), false)
            ]
        );
    }

    #[test]
    fn one_shot_cancellation_interrupts_delay_and_releases_owned_keys() {
        let (control, receiver) = mpsc::channel();
        let (record, events) = mpsc::channel();
        let mut sequence = held_sequence();
        sequence.while_held[1] = MacroStep::Delay { millis: 60_000 };
        let worker = thread::spawn(move || run(&sequence, &receiver, false, RecordedKeys(record)));
        let down = events.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!((down.0, down.1), (key(0xe4), true));
        control.send(Signal::Cancel).unwrap();
        worker.join().unwrap();
        let remaining: Vec<_> = events.try_iter().map(|event| (event.0, event.1)).collect();
        assert_eq!(remaining, [(key(0xe4), false)]);
    }

    #[test]
    fn macro_preserves_right_control_delay_and_release_order() {
        let (control, receiver) = mpsc::channel();
        let (record, events) = mpsc::channel();
        let worker =
            thread::spawn(move || run(&held_sequence(), &receiver, true, RecordedKeys(record)));
        let ctrl_down = events.recv_timeout(Duration::from_secs(1)).unwrap();
        let f8_down = events.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!((ctrl_down.0, ctrl_down.1), (key(0xe4), true));
        assert_eq!((f8_down.0, f8_down.1), (key(0x41), true));
        assert!(f8_down.2.duration_since(ctrl_down.2) >= Duration::from_millis(45));
        assert!(
            events.try_recv().is_err(),
            "keys stay held until physical release"
        );
        control.send(Signal::Release).unwrap();
        let f8_up = events.recv_timeout(Duration::from_secs(1)).unwrap();
        let ctrl_up = events.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!((f8_up.0, f8_up.1), (key(0x41), false));
        assert_eq!((ctrl_up.0, ctrl_up.1), (key(0xe4), false));
        assert!(ctrl_up.2.duration_since(f8_up.2) >= Duration::from_millis(45));
        worker.join().unwrap();
        events.try_recv().unwrap_err();
    }

    #[test]
    fn macro_cancel_during_delay_releases_owned_keys_without_late_f8() {
        let (control, receiver) = mpsc::channel();
        let (record, events) = mpsc::channel();
        let mut sequence = held_sequence();
        sequence.while_held[1] = MacroStep::Delay { millis: 60_000 };
        let worker = thread::spawn(move || run(&sequence, &receiver, true, RecordedKeys(record)));
        let down = events.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!((down.0, down.1), (key(0xe4), true));
        control.send(Signal::Cancel).unwrap();
        let up = events.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!((up.0, up.1), (key(0xe4), false));
        worker.join().unwrap();
        events.try_recv().unwrap_err();
    }

    #[test]
    fn macro_quick_release_never_presses_a_key_after_release() {
        let (control, receiver) = mpsc::channel();
        let (record, events) = mpsc::channel();
        let mut sequence = held_sequence();
        sequence.while_held[1] = MacroStep::Delay { millis: 60_000 };
        let worker = thread::spawn(move || run(&sequence, &receiver, true, RecordedKeys(record)));
        let down = events.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!((down.0, down.1), (key(0xe4), true));
        control.send(Signal::Release).unwrap();
        let up = events.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!((up.0, up.1), (key(0xe4), false));
        worker.join().unwrap();
        events.try_recv().unwrap_err();
    }

    #[test]
    fn repeating_macro_stops_at_release_and_runs_release_phase_once() {
        let (control, receiver) = mpsc::channel();
        let (record, events) = mpsc::channel();
        let sequence = Macro {
            on_press: vec![MacroStep::KeyDown(key(0xe0))],
            while_held: vec![
                MacroStep::KeyDown(key(4)),
                MacroStep::KeyUp(key(4)),
                MacroStep::Delay { millis: 5 },
            ],
            on_release: vec![MacroStep::KeyUp(key(0xe0))],
            repeat_while_held: true,
            ..Macro::default()
        };
        let worker = thread::spawn(move || run(&sequence, &receiver, true, RecordedKeys(record)));
        let mut seen = Vec::new();
        for _ in 0..5 {
            let event = events.recv_timeout(Duration::from_secs(1)).unwrap();
            seen.push((event.0, event.1));
        }
        assert_eq!(
            seen,
            [
                (key(0xe0), true),
                (key(4), true),
                (key(4), false),
                (key(4), true),
                (key(4), false)
            ]
        );
        control.send(Signal::Release).unwrap();
        worker.join().unwrap();
        let remaining: Vec<_> = events.try_iter().map(|event| (event.0, event.1)).collect();
        assert_eq!(remaining.last(), Some(&(key(0xe0), false)));
        assert_eq!(
            remaining
                .iter()
                .filter(|event| **event == (key(0xe0), false))
                .count(),
            1
        );
        let a_down = remaining
            .iter()
            .filter(|event| **event == (key(4), true))
            .count();
        let a_up = remaining
            .iter()
            .filter(|event| **event == (key(4), false))
            .count();
        assert_eq!(a_down, a_up, "release also balances an interrupted loop");
    }

    #[test]
    fn cancel_in_release_phase_skips_remaining_actions_and_balances_keys() {
        let (control, receiver) = mpsc::channel();
        let (record, events) = mpsc::channel();
        let sequence = Macro {
            on_press: vec![MacroStep::KeyDown(key(0xe4))],
            on_release: vec![
                MacroStep::KeyDown(key(4)),
                MacroStep::Delay { millis: 60_000 },
                MacroStep::KeyDown(key(5)),
            ],
            ..Macro::default()
        };
        let worker = thread::spawn(move || run(&sequence, &receiver, true, RecordedKeys(record)));
        let _ctrl_down = events.recv_timeout(Duration::from_secs(1)).unwrap();
        control.send(Signal::Release).unwrap();
        let a_down = events.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!((a_down.0, a_down.1), (key(4), true));
        control.send(Signal::Cancel).unwrap();
        worker.join().unwrap();
        let remaining: Vec<_> = events.try_iter().map(|event| (event.0, event.1)).collect();
        assert_eq!(remaining, [(key(4), false), (key(0xe4), false)]);
    }
}
