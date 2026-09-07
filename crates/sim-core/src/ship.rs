//! Ship objects and related code

use crate::bodies::CentralBody;
use crate::integrate::{integrate, two_body};
use crate::plan::{FlightPlan, Maneuver};
use crate::time::{Clock, TimeStep, seconds_since};
use crate::vectors::{Orbit, StateVector};
use glam::DVec3;
struct Ship {
    /// Gotta have a name
    name: String,
    /// Position and Velocity at time on clock
    current_state: StateVector,
    /// Center of the Ship's current orbit
    center: CentralBody,
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
        center: CentralBody,
        clock: Clock,
        transponder_id: String,
        mass: f64,
        max_accel: f64,
    ) -> Self {
        Self {
            name,
            current_state,
            center,
            clock,
            transponder_id,
            mass,
            max_accel,
        }
    }

    pub fn fly(&mut self, plan: &FlightPlan) -> StateVector {
        let reference = self.clock.now();
        let total_time: f64 = plan
            .maneuvers()
            .map(|m| match m {
                Maneuver::Burn(b) => seconds_since(reference, b.start()) + b.duration(),
            })
            .fold(0.0, f64::max);
        let new_state = integrate(
            self.current_state,
            0.0,
            total_time,
            substep_calculator(total_time),
            |t, s| {
                two_body(self.center.mu(), s)
                    + plan
                        .maneuvers()
                        .map(|m| match m {
                            Maneuver::Burn(b) => b.accel_at(reference, t),
                        })
                        .sum::<DVec3>()
            },
        );
        self.clock.advance(TimeStep::Seconds(total_time));
        self.current_state = new_state;
        new_state
    }

    pub fn orbit(&self) -> Orbit {
        Orbit::from_state(self.current_state, self.center, self.clock.now())
    }
}
/// Helper to calculate substeps from the total time
/// Currently a stub, will need to see how substeps effect performance before I implement it.
/// May end up as a GM mode setting
fn substep_calculator(_total: f64) -> f64 {
    60.0
}
