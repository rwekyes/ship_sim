//! Time related code

use hifitime::{Duration, Epoch, TimeScale, TimeUnits};
use std::sync::LazyLock;
/// Constant for universal start time, Jan 1, 2000, Noon, TT
pub static J2000: LazyLock<Epoch> =
    LazyLock::new(|| Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, TimeScale::TT));
/// Main game clock
#[derive(Debug)]
pub struct Clock {
    now: Epoch,
}

impl Clock {
    pub fn new(start: Epoch) -> Self {
        Self { now: start }
    }

    pub fn now(&self) -> Epoch {
        self.now
    }

    pub fn advance(&mut self, step: TimeStep) {
        let dt: Duration = match step {
            TimeStep::Seconds(s) => s.seconds(),
            TimeStep::Round => 15.seconds(),
            TimeStep::Minute => 1.minutes(),
            TimeStep::Hour => 1.hours(),
            TimeStep::HalfDay => 12.hours(),
            TimeStep::FullDay => 1.days(),
        };
        self.now += dt;
    }
}
/// Game-sized steps, convenient for quick buttons
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeStep {
    Seconds(f64),
    Round,
    Minute,
    Hour,
    HalfDay,
    FullDay,
}

pub fn seconds_since(epoch: Epoch, t: Epoch) -> f64 {
    (t - epoch).to_seconds()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seconds_since() {
        let e: Epoch = *J2000;
        let mut c: Clock = Clock::new(e);
        c.advance(TimeStep::HalfDay);
        assert_eq!(43200.0, seconds_since(e, c.now()));
    }

    #[test]
    fn test_seconds_since_reversed_order() {
        let e: Epoch = *J2000;
        let mut c: Clock = Clock::new(e);
        c.advance(TimeStep::HalfDay);
        assert!(seconds_since(c.now(), e) < 0.0);
    }

    #[test]
    fn test_repeated_advances() {
        let e: Epoch = *J2000;
        let mut c1: Clock = Clock::new(e);
        c1.advance(TimeStep::HalfDay);
        c1.advance(TimeStep::HalfDay);
        let mut c2: Clock = Clock::new(e);
        c2.advance(TimeStep::FullDay);
        assert_eq!(c1.now(), c2.now());
    }

    #[test]
    fn test_utc_to_tt() {
        let e: Epoch = *J2000;
        assert_eq!(
            e,
            Epoch::from_gregorian_utc(2000, 1, 1, 11, 58, 55, 816_000_000)
        );
    }
}
