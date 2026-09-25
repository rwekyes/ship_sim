//! Ship objects and related code

use crate::bodies::CentralBody;
use crate::integrate::{integrate, two_body};
use crate::plan::{FlightPlan, Maneuver};
use crate::time::{Clock, TimeStep};
use crate::vectors::{Orbit, StateVector};
use hifitime::Epoch;
pub struct Ship {
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
        for m in plan.maneuvers() {
            let reference = self.clock.now();
            let start = match m {
                Maneuver::Burn(b) => b.start(),
            };
            if start > reference {
                let duration = (start - reference).to_seconds();
                self.current_state = integrate(
                    self.current_state,
                    0.0,
                    duration,
                    substep_calculator(duration),
                    |_t, s| two_body(self.center.mu(), s),
                );
                self.clock.advance(TimeStep::Seconds(duration));
            }
            let duration = match m {
                Maneuver::Burn(b) => b.duration(),
            };
            let new_state = integrate(
                self.current_state,
                0.0,
                duration,
                substep_calculator(duration),
                |t, s| {
                    two_body(self.center.mu(), s)
                        + match m {
                            Maneuver::Burn(b) => b.accel_at(reference, t),
                        }
                },
            );
            self.current_state = new_state;
            self.clock.advance(TimeStep::Seconds(duration));
        }
        self.current_state
    }

    pub fn orbit(&self) -> Orbit {
        Orbit::from_state(self.current_state, self.center, self.clock.now())
    }
    pub fn now(&self) -> Epoch {
        self.clock.now()
    }
}
/// Helper to calculate substeps from the total time
/// Currently a stub, will need to see how substeps effect performance before I implement it.
/// May end up as a GM mode setting
fn substep_calculator(_total: f64) -> f64 {
    60.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::burns::Burn;
    use crate::plan::{FlightPlan, Maneuver};
    use crate::time::J2000;
    use glam::DVec3;
    #[test]
    fn test_flight() {
        let test_burn1 = Burn::new(*J2000, 6000.0, 100.0, DVec3::new(0.11, -4.2, 0.77777)).unwrap();
        let test_burn2 = Burn::new(
            *J2000 + 6000.0,
            6000.0,
            100.0,
            DVec3::new(-0.11, 4.2, -0.77777),
        )
        .unwrap();
        let test_burn3 = Burn::new(
            *J2000 + 12000.0,
            6000.0,
            100.0,
            DVec3::new(-0.11, 4.2, -0.77777),
        )
        .unwrap();
        let test_burn4 = Burn::new(
            *J2000 + 18000.0,
            6000.0,
            100.0,
            DVec3::new(0.11, -4.2, 0.77777),
        )
        .unwrap();
        let maneuvers = vec![
            Maneuver::Burn(test_burn1),
            Maneuver::Burn(test_burn2),
            Maneuver::Burn(test_burn3),
            Maneuver::Burn(test_burn4),
        ];
        let test_plan = FlightPlan::new(maneuvers);
        let name = String::from("Test Ship");
        let initial_position = DVec3::new(1.0, 1.0, 1.0);
        let initial_velocity = DVec3::new(1.0, 1.0, 1.0);
        let current_state = StateVector {
            position: initial_position,
            velocity: initial_velocity,
        };
        let center = CentralBody::None;
        let clock = Clock::new(*J2000);
        let transponder_id = String::from("123456789");
        let mass = 1.0;
        let max_accel = 1000.0;
        let mut test_ship = Ship::new(
            name,
            current_state,
            center,
            clock,
            transponder_id,
            mass,
            max_accel,
        );
        let final_state = test_ship.fly(&test_plan);
        let expected_position = initial_position + initial_velocity * 24000.0;
        let expected_velocity = initial_velocity;
        let position_distance = final_state.position.distance(expected_position);
        let velocity_distance = final_state.velocity.distance(expected_velocity);
        assert!(
            position_distance <= 1e-6,
            "Distance between initial and final position is {}, greater than tolerance of 1e-6",
            position_distance
        );
        assert!(
            velocity_distance <= 1e-8,
            "Distance between initial and final velocity is {}, greater than tolerance of 1e-8",
            velocity_distance
        );
    }
}
