use crate::{Modifiers, Pixels, Point, ScrollDelta, ScrollWheelEvent, px};
use std::time::Duration;

/// How far the pointer may drift during one scroll and still be the same
/// gesture: hand jitter on a resting mouse, not a move to another control.
const JITTER: Pixels = px(6.);

pub(super) struct Motion {
    event: ScrollWheelEvent,
    distance: Point<f32>,
    emitted: Point<f32>,
    started: std::time::Instant,
    duration: Duration,
}

impl Motion {
    /// Whether input at `position` with `modifiers` belongs to this motion.
    pub(super) fn holds(&self, position: Point<Pixels>, modifiers: Modifiers) -> bool {
        let drift = position - self.event.position;
        modifiers == self.event.modifiers && drift.x.abs() <= JITTER && drift.y.abs() <= JITTER
    }
}

#[derive(Default)]
pub(crate) struct Wheel {
    pub(super) motion: Option<Motion>,
}

impl Wheel {
    pub(super) fn push(&mut self, event: ScrollWheelEvent, duration: Duration) {
        let ScrollDelta::Lines(delta) = event.delta else {
            unreachable!()
        };
        let mut distance = delta;
        if let Some(previous) = &self.motion
            && previous.holds(event.position, event.modifiers)
        {
            let pending = previous.distance - previous.emitted;
            // Reversing cancels only that axis, not an independent other axis.
            for (next, old) in [(&mut distance.x, pending.x), (&mut distance.y, pending.y)] {
                if *next == 0. || next.signum() == old.signum() {
                    *next += old;
                }
            }
        }
        self.motion = Some(Motion {
            event,
            distance,
            emitted: Point::default(),
            started: std::time::Instant::now(),
            duration,
        });
    }

    pub(super) fn advance(
        &mut self,
        now: std::time::Instant,
        reduce_motion: bool,
    ) -> Option<ScrollWheelEvent> {
        let motion = self.motion.as_mut()?;
        let progress = if reduce_motion {
            1.
        } else {
            (now.duration_since(motion.started).as_secs_f32() / motion.duration.as_secs_f32())
                .min(1.)
        };
        let next = motion.distance * (1. - (1. - progress).powi(3));
        let mut event = motion.event.clone();
        event.delta = ScrollDelta::Lines(next - motion.emitted);
        motion.emitted = next;
        if progress == 1. {
            self.motion = None;
        }
        Some(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bursts_preserve_distance_and_reversal_cancels_only_its_axis() {
        let mut wheel = Wheel::default();
        let duration = Duration::from_millis(120);
        for _ in 0..100 {
            wheel.push(
                ScrollWheelEvent {
                    delta: ScrollDelta::Lines(crate::point(-1., -2.)),
                    ..Default::default()
                },
                duration,
            );
        }
        let started = wheel.motion.as_ref().unwrap().started;
        let half = wheel.advance(started + duration / 2, false).unwrap();
        let ScrollDelta::Lines(half) = half.delta else {
            panic!()
        };
        assert!(half.x < 0. && half.x > -100.);
        wheel.push(
            ScrollWheelEvent {
                delta: ScrollDelta::Lines(crate::point(0., 1.)),
                ..Default::default()
            },
            duration,
        );
        let started = wheel.motion.as_ref().unwrap().started;
        let final_event = wheel.advance(started + duration, false).unwrap();
        let ScrollDelta::Lines(rest) = final_event.delta else {
            panic!()
        };
        assert_eq!(half.x + rest.x, -100.);
        assert_eq!(rest.y, 1.);
        assert!(wheel.motion.is_none());
    }

    #[test]
    fn jitter_keeps_pending_distance_but_a_real_move_does_not() {
        let duration = Duration::from_millis(120);
        let at = |x: f32| ScrollWheelEvent {
            position: crate::point(px(x), px(10.)),
            delta: ScrollDelta::Lines(crate::point(0., -3.)),
            ..Default::default()
        };
        let mut wheel = Wheel::default();
        wheel.push(at(10.), duration);
        wheel.push(at(13.), duration);
        assert_eq!(wheel.motion.as_ref().unwrap().distance.y, -6.);
        wheel.push(at(40.), duration);
        assert_eq!(wheel.motion.as_ref().unwrap().distance.y, -3.);
        let motion = wheel.motion.as_ref().unwrap();
        assert!(motion.holds(crate::point(px(44.), px(15.)), Modifiers::default()));
        assert!(!motion.holds(crate::point(px(47.), px(10.)), Modifiers::default()));
    }
}
