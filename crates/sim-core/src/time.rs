use hifitime::Epoch;
use hifitime::prelude::*;
/// Time related code
pub struct Clock {
    now: Epoch,
}

impl Clock {
    pub fn new(mut self, start: Epoch) -> Self {
        self.now = start;
        self
    }

    pub fn now(&self) -> Epoch {
        self.now
    }

    pub fn advance(&mut self, step: TimeStep) {
        match step {
            TimeStep::Round => self.now = self.now + 15.seconds(),
            TimeStep::Minute => self.now = self.now + 1.minutes(),
            TimeStep::Hour => self.now = self.now + 1.hours(),
            TimeStep::HalfDay => self.now = self.now + 12.hours(),
            TimeStep::FullDay => self.now = self.now + 1.days(),
        }
    }

}

pub enum TimeStep {
    Round,
    Minute,
    Hour,
    HalfDay,
    FullDay,
}