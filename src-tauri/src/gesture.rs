//! Pure gesture state machine: three-finger tap detection.
//!
//! No I/O, no platform types, no clock reads: callers pass millisecond
//! timestamps in. Every rule from the charter is enforced here and covered by
//! unit tests at the bottom of the file.
//!
//! Rules (charter):
//! - a candidate starts on the first frame with `contacts.len() >= 3`
//! - `peak_fingers` is the max contact count during the candidate
//! - the candidate ends when the count drops below 3 (fingers lifted)
//! - `MiddleClick` is emitted only when all hold:
//!   `peak_fingers == finger_count`, duration <= `max_duration_ms`,
//!   centroid travel <= `max_movement`, cooldown elapsed.
//!
//! Travel is measured as the largest centroid displacement from the starting
//! centroid across all frames of the candidate. That is the strictest reading of
//! "centroid travel from start" (a scroll that returns near its origin still
//! counts as movement) and it cannot be fooled by a moving average.

/// Smallest sane finger count accepted by the state machine.
pub const MIN_FINGER_COUNT: u8 = 2;
/// Largest sane finger count accepted by the state machine.
pub const MAX_FINGER_COUNT: u8 = 5;

/// A single finger position in normalized trackpad coordinates (0.0..=1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    pub x: f32,
    pub y: f32,
}

impl Contact {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// A position we can trust for movement accounting. Garbage read through an
    /// unknown contact stride fails this and is ignored by the centroid.
    pub fn is_plausible(&self) -> bool {
        crate::contact::coords_plausible(self.x, self.y)
    }
}

/// One contact frame delivered by the platform layer.
#[derive(Debug, Clone, Copy)]
pub struct Frame<'a> {
    /// Monotonic timestamp in milliseconds.
    pub t_ms: u64,
    pub contacts: &'a [Contact],
}

impl<'a> Frame<'a> {
    pub fn new(t_ms: u64, contacts: &'a [Contact]) -> Self {
        Self { t_ms, contacts }
    }
}

/// Result of feeding one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    MiddleClick,
    None,
}

/// Tunables of the tap detector. Mirrors `config::GestureConfig`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    pub finger_count: u8,
    pub max_duration_ms: u64,
    pub max_movement: f32,
    pub cooldown_ms: u64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            finger_count: 3,
            max_duration_ms: 400,
            max_movement: 0.08,
            cooldown_ms: 250,
        }
    }
}

impl Params {
    /// Clamp to the documented safe ranges. Non-finite movement falls back to
    /// the default so the machine can never compare against NaN.
    pub fn sanitized(self) -> Self {
        let max_movement = if self.max_movement.is_finite() {
            self.max_movement.clamp(0.02, 0.20)
        } else {
            Params::default().max_movement
        };
        Self {
            finger_count: self.finger_count.clamp(MIN_FINGER_COUNT, MAX_FINGER_COUNT),
            max_duration_ms: self.max_duration_ms.clamp(150, 800),
            max_movement,
            cooldown_ms: self.cooldown_ms.clamp(100, 1000),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    start_ms: u64,
    peak_fingers: usize,
    /// `None` while no frame has produced a plausible centroid yet.
    start_centroid: Option<(f32, f32)>,
    max_travel: f32,
}

/// The tap detector. Feed frames in timestamp order.
#[derive(Debug, Clone)]
pub struct Machine {
    params: Params,
    candidate: Option<Candidate>,
    last_emit_ms: Option<u64>,
}

impl Machine {
    pub fn new(params: Params) -> Self {
        Self {
            params: params.sanitized(),
            candidate: None,
            last_emit_ms: None,
        }
    }

    pub fn params(&self) -> Params {
        self.params
    }

    /// Update tunables. A real change drops any in-flight candidate (its old
    /// thresholds no longer apply) but keeps the cooldown, so a config tweak
    /// cannot be used to double fire.
    pub fn set_params(&mut self, params: Params) {
        let next = params.sanitized();
        if next != self.params {
            self.params = next;
            self.candidate = None;
        }
    }

    /// Drop all state, including the cooldown. Used when the listener restarts.
    pub fn reset(&mut self) {
        self.candidate = None;
        self.last_emit_ms = None;
    }

    /// Feed one frame and report what should happen.
    pub fn push_frame(&mut self, frame: Frame<'_>) -> Outcome {
        let now = frame.t_ms;
        let fingers = frame.contacts.len();

        if fingers >= 3 {
            match self.candidate {
                Some(mut candidate) => {
                    candidate.peak_fingers = candidate.peak_fingers.max(fingers);
                    if let Some(current) = centroid(frame.contacts) {
                        match candidate.start_centroid {
                            Some(start) => {
                                let travel = distance(start, current);
                                if travel > candidate.max_travel {
                                    candidate.max_travel = travel;
                                }
                            }
                            // First plausible frame after unreadable ones: this
                            // becomes the reference instead of a bogus origin.
                            None => candidate.start_centroid = Some(current),
                        }
                    }
                    self.candidate = Some(candidate);
                }
                None => {
                    // A candidate starts even when positions are unreadable: the
                    // tap must not be rejected just because the stride is wrong.
                    self.candidate = Some(Candidate {
                        start_ms: now,
                        peak_fingers: fingers,
                        start_centroid: centroid(frame.contacts),
                        max_travel: 0.0,
                    });
                }
            }
            return Outcome::None;
        }

        // Fewer than three fingers: this frame ends (or never starts) a tap.
        let Some(candidate) = self.candidate.take() else {
            return Outcome::None;
        };

        let duration_ms = now.saturating_sub(candidate.start_ms);
        let correct_fingers = candidate.peak_fingers == usize::from(self.params.finger_count);
        let short_enough = duration_ms <= self.params.max_duration_ms;
        let steady_enough = candidate.max_travel <= self.params.max_movement;
        let cooldown_elapsed = match self.last_emit_ms {
            None => true,
            Some(previous) => now.saturating_sub(previous) >= self.params.cooldown_ms,
        };

        if correct_fingers && short_enough && steady_enough && cooldown_elapsed {
            self.last_emit_ms = Some(now);
            Outcome::MiddleClick
        } else {
            Outcome::None
        }
    }

    /// Convenience wrapper for callers that do not want to build a `Frame`.
    pub fn push(&mut self, t_ms: u64, contacts: &[Contact]) -> Outcome {
        self.push_frame(Frame::new(t_ms, contacts))
    }
}

impl Default for Machine {
    fn default() -> Self {
        Self::new(Params::default())
    }
}

fn centroid(contacts: &[Contact]) -> Option<(f32, f32)> {
    let mut sx = 0.0_f32;
    let mut sy = 0.0_f32;
    let mut count = 0usize;

    for contact in contacts {
        if contact.is_plausible() {
            sx += contact.x;
            sy += contact.y;
            count += 1;
        }
    }

    if count == 0 {
        return None;
    }
    let n = count as f32;
    Some((sx / n, sy / n))
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three(x: f32, y: f32) -> Vec<Contact> {
        vec![
            Contact::new(x, y),
            Contact::new(x + 0.05, y),
            Contact::new(x, y + 0.05),
        ]
    }

    fn four(x: f32, y: f32) -> Vec<Contact> {
        let mut contacts = three(x, y);
        contacts.push(Contact::new(x + 0.05, y + 0.05));
        contacts
    }

    fn two(x: f32, y: f32) -> Vec<Contact> {
        vec![Contact::new(x, y), Contact::new(x + 0.05, y)]
    }

    fn machine() -> Machine {
        Machine::new(Params::default())
    }

    #[test]
    fn three_finger_tap_fires() {
        let mut m = machine();
        assert_eq!(m.push(0, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(60, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(120, &[]), Outcome::MiddleClick);
    }

    #[test]
    fn four_finger_tap_does_not_fire() {
        let mut m = machine();
        assert_eq!(m.push(0, &four(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(80, &four(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(140, &[]), Outcome::None);
    }

    #[test]
    fn two_finger_tap_does_not_fire() {
        let mut m = machine();
        assert_eq!(m.push(0, &two(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(80, &two(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(140, &[]), Outcome::None);
    }

    #[test]
    fn movement_beyond_tolerance_does_not_fire() {
        let mut m = machine();
        assert_eq!(m.push(0, &three(0.2, 0.5)), Outcome::None);
        assert_eq!(m.push(60, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(120, &[]), Outcome::None);
    }

    #[test]
    fn movement_back_to_origin_does_not_fire() {
        let mut m = machine();
        assert_eq!(m.push(0, &three(0.2, 0.5)), Outcome::None);
        assert_eq!(m.push(40, &three(0.6, 0.5)), Outcome::None);
        assert_eq!(m.push(80, &three(0.2, 0.5)), Outcome::None);
        assert_eq!(m.push(120, &[]), Outcome::None);
    }

    #[test]
    fn long_hold_does_not_fire() {
        let mut m = machine();
        assert_eq!(m.push(0, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(200, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(600, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(650, &[]), Outcome::None);
    }

    #[test]
    fn cooldown_blocks_double_fire() {
        let mut m = machine();
        assert_eq!(m.push(0, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(80, &[]), Outcome::MiddleClick);

        // Second tap starts 100 ms after the first fired and ends 220 ms after.
        assert_eq!(m.push(180, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(300, &[]), Outcome::None);

        // Third tap is outside the 250 ms cooldown window.
        assert_eq!(m.push(400, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(520, &[]), Outcome::MiddleClick);
    }

    #[test]
    fn cooldown_below_minimum_is_clamped_to_100ms() {
        let mut m = Machine::new(Params {
            cooldown_ms: 0,
            ..Params::default()
        });
        assert_eq!(m.params().cooldown_ms, 100);

        assert_eq!(m.push(0, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(50, &[]), Outcome::MiddleClick);

        // 60 ms after the first emit: blocked by the clamped cooldown.
        assert_eq!(m.push(60, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(110, &[]), Outcome::None);

        // 200 ms after the first emit: allowed.
        assert_eq!(m.push(200, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(250, &[]), Outcome::MiddleClick);
    }

    #[test]
    fn resting_finger_frames_still_fire() {
        let mut m = machine();
        for t in 0..=300 {
            let contacts = three(0.5, 0.5);
            let expected = Outcome::None;
            assert_eq!(m.push(t, &contacts), expected, "frame at {t}");
        }
        assert_eq!(m.push(320, &[]), Outcome::MiddleClick);
    }

    #[test]
    fn lifting_order_does_not_matter() {
        let mut m = machine();
        // Same positions, different order: no identity tracking is involved.
        let reordered = vec![
            Contact::new(0.5, 0.55),
            Contact::new(0.55, 0.5),
            Contact::new(0.5, 0.5),
        ];
        assert_eq!(m.push(0, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(70, &reordered), Outcome::None);
        // Dropping below three fingers ends the candidate, so the click fires on
        // the first partial-lift frame; the rest of the lift is ignored.
        let partial = vec![Contact::new(0.5, 0.55), Contact::new(0.5, 0.5)];
        assert_eq!(m.push(110, &partial), Outcome::MiddleClick);
        assert_eq!(m.push(150, &[]), Outcome::None);
    }

    #[test]
    fn peak_of_four_then_three_does_not_fire() {
        let mut m = machine();
        assert_eq!(m.push(0, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(40, &four(0.5, 0.5)), Outcome::None);
        let three_again = three(0.5, 0.5);
        assert_eq!(m.push(80, three_again.as_slice()), Outcome::None);
        assert_eq!(m.push(120, &[]), Outcome::None);
    }

    #[test]
    fn intermediate_lift_breaks_the_candidate() {
        let mut m = machine();
        assert_eq!(m.push(0, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(40, &[]), Outcome::MiddleClick);
        // New candidate starts after the lift.
        assert_eq!(m.push(500, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(560, &[]), Outcome::MiddleClick);
    }

    #[test]
    fn reset_clears_candidate_and_cooldown() {
        let mut m = machine();
        assert_eq!(m.push(0, &three(0.5, 0.5)), Outcome::None);
        m.reset();
        assert_eq!(m.push(50, &[]), Outcome::None);
        // Cooldown is gone, so a tap right after reset fires.
        assert_eq!(m.push(60, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(120, &[]), Outcome::MiddleClick);
    }

    #[test]
    fn param_change_drops_candidate_but_keeps_cooldown() {
        let mut m = machine();
        assert_eq!(m.push(0, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(80, &[]), Outcome::MiddleClick);

        m.set_params(Params {
            finger_count: 4,
            ..Params::default()
        });
        assert_eq!(m.push(100, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(120, &[]), Outcome::None);
        assert_eq!(m.push(400, &four(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(460, &[]), Outcome::MiddleClick);
    }

    #[test]
    fn sanitize_clamps_params() {
        let raw = Params {
            finger_count: 9,
            max_duration_ms: 5000,
            max_movement: 5.0,
            cooldown_ms: 90_000,
        };
        let p = raw.sanitized();
        assert_eq!(p.finger_count, MAX_FINGER_COUNT);
        assert_eq!(p.max_duration_ms, 800);
        assert!((p.max_movement - 0.20).abs() < f32::EPSILON);
        assert_eq!(p.cooldown_ms, 1000);

        let nan = Params {
            max_movement: f32::NAN,
            ..Params::default()
        };
        assert_eq!(nan.sanitized().max_movement, Params::default().max_movement);
    }

    #[test]
    fn unreadable_positions_do_not_reject_a_tap() {
        let mut m = machine();
        let junk = vec![
            Contact::new(f32::NAN, f32::NAN),
            Contact::new(f32::INFINITY, 0.5),
            Contact::new(-9.0, 9.0),
        ];
        assert_eq!(m.push(0, &junk), Outcome::None);
        assert_eq!(m.push(60, &junk), Outcome::None);
        assert_eq!(m.push(120, &[]), Outcome::MiddleClick);
    }

    #[test]
    fn unreadable_frames_are_skipped_for_movement() {
        let mut m = machine();
        assert_eq!(m.push(0, &three(0.30, 0.50)), Outcome::None);

        let junk = vec![
            Contact::new(f32::NAN, f32::NAN),
            Contact::new(f32::NAN, 0.5),
            Contact::new(0.5, f32::INFINITY),
        ];
        assert_eq!(m.push(40, &junk), Outcome::None);

        // 0.02 travel from the last readable centroid, not a huge jump from junk.
        assert_eq!(m.push(80, &three(0.32, 0.50)), Outcome::None);
        assert_eq!(m.push(120, &[]), Outcome::MiddleClick);
    }

    #[test]
    fn unreadable_start_then_plausible_frames_use_first_readable_origin() {
        let mut m = machine();
        let junk = vec![
            Contact::new(f32::NAN, 0.5),
            Contact::new(0.5, f32::NAN),
            Contact::new(f32::INFINITY, 0.5),
        ];
        assert_eq!(m.push(0, &junk), Outcome::None);
        assert_eq!(m.push(50, &three(0.5, 0.5)), Outcome::None);
        assert_eq!(m.push(90, &three(0.51, 0.5)), Outcome::None);
        assert_eq!(m.push(130, &[]), Outcome::MiddleClick);
    }

    #[test]
    fn empty_frames_are_harmless() {
        let mut m = machine();
        for t in 0..10 {
            assert_eq!(m.push(t, &[]), Outcome::None);
        }
    }

    #[test]
    fn five_fingers_do_not_fire_when_finger_count_is_three() {
        let mut m = machine();
        let mut five = four(0.5, 0.5);
        five.push(Contact::new(0.6, 0.6));
        assert_eq!(m.push(0, &five), Outcome::None);
        assert_eq!(m.push(80, &[]), Outcome::None);
    }

    #[test]
    fn out_of_order_timestamps_saturate_instead_of_wrapping() {
        let mut m = machine();
        assert_eq!(m.push(200, &three(0.5, 0.5)), Outcome::None);
        // saturating_sub keeps the duration at 0 rather than wrapping to ~u64::MAX.
        assert_eq!(m.push(100, &[]), Outcome::MiddleClick);
    }
}
