//! Ship objects and related code

use crate::integrate::integrate;
use crate::plan::{FlightPlan, Maneuver};
use crate::time::{Clock, TimeStep};
use crate::vectors::StateVector;
use glam::DVec3;
struct Ship {
    /// meters per second squared
    max_accel: f64,

    current_state: StateVector,

    clock: Clock,
}
impl Ship {
    pub fn new(max_accel: f64, current_state: StateVector, clock: Clock) -> Self {
        Self {
            max_accel,
            current_state,
            clock,
        }
    }

    pub fn fly(&mut self, plan: FlightPlan) -> StateVector {
        let total_time: f64 = plan
            .maneuvers()
            .map(|m| match m {
                Maneuver::Burn(b) => b.duration(),
            })
            .sum();
        let new_state = integrate(
            self.current_state,
            0.0,
            total_time,
            substep_calculator(total_time),
            |t, _s| {
                plan.maneuvers()
                    .map(|m| match m {
                        Maneuver::Burn(b) => b.accel_at(self.clock.now(), t),
                    })
                    .sum::<DVec3>()
            },
        );
        self.clock.advance(TimeStep::Seconds(total_time));
        self.current_state = new_state;
        new_state
    }
}
/// Helper to calculate substeps from the total steps
/// Currently a stub, will need to see how substeps effect performance before I implement it.
fn substep_calculator(_total: f64) -> f64 {
    1.0
}
