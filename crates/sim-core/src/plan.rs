//! Flight plan containers
//! Serves both the planner and the UI

use crate::burns::Burn;
use serde::{Deserialize, Serialize};
/// May add variants for game reasons, like Waypoint
#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub enum Maneuver {
    Burn(Burn),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightPlan {
    maneuvers: Vec<Maneuver>,
}

impl FlightPlan {
    pub fn maneuvers(&self) -> impl Iterator<Item = &Maneuver> {
        self.maneuvers.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrate::integrate;
    use crate::time::J2000;
    use crate::vectors::StateVector;
    use glam::DVec3;
    use hifitime::TimeUnits;

    #[test]
    fn one_d_brachistochrone() {
        let maneuvers = vec![
            Maneuver::Burn(Burn::new(*J2000, 10000.0, 7.0, DVec3::new(0.0, 0.0, 1.0)).unwrap()),
            Maneuver::Burn(
                Burn::new(
                    *J2000 + 10000.0.seconds(),
                    10000.0,
                    7.0,
                    DVec3::new(0.0, 0.0, -1.0),
                )
                .unwrap(),
            ),
        ];

        let plan = FlightPlan { maneuvers };

        let state = StateVector {
            position: DVec3::new(
                -2.65025768897131e7,
                1.44693955627991e8,
                -1.704331902042031e2,
            ) * 1e3,
            velocity: DVec3::ZERO,
        };

        let final_state = integrate(state, 0.0, 20000.0, 10.0, |t, _s| {
            plan.maneuvers()
                .map(|m| match m {
                    Maneuver::Burn(b) => b.accel_at(*J2000, t),
                })
                .sum::<DVec3>()
        });
        let velocity_diff = state.velocity.distance(final_state.velocity);
        let position_diff = state.position.distance(final_state.position);
        assert!(
            velocity_diff.abs() < 1e-7,
            "Difference in final and predicted velocities {} is greater than 1e-7",
            velocity_diff
        );
        assert!(
            (position_diff - 7e8).abs() < 1e-7,
            "Difference in final and predicted positions {} is greater than 1e-7",
            position_diff
        );
    }
}
