//! Ship objects and related code

use crate::integrate::integrate;
use crate::plan::{FlightPlan, Maneuver};
use crate::time::{Clock, TimeStep};
use crate::vectors::{Orbit, StateVector};
use glam::DVec3;
struct Ship {
    /// Gotta have a name
    name: String,
    /// Position and Velocity at time on clock
    current_state: StateVector,
    /// Orbit object for ease of access to orbital data
    orbit: Orbit,
    /// Ship's current time
    clock: Clock,
    /// String for now, may update to something cool like hex later
    transponder_id: String,
    /// kilograms
    mass: f64,
    /// meters per second squared
    max_accel: f64,
}
impl Ship {
    pub fn new(
        name: String,
        current_state: StateVector,
        orbit: Orbit,
        clock: Clock,
        transponder_id: String,
        mass: f64,
        max_accel: f64,
    ) -> Self {
        Self {
            name,
            current_state,
            orbit,
            clock,
            transponder_id,
            mass,
            max_accel,
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
/// May end up as a GM mode setting
fn substep_calculator(_total: f64) -> f64 {
    1.0
}
