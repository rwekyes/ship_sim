//! Flight Solvers

use crate::burns::{Burn, BurnError};
use crate::orbits::KeplerError;
use crate::plan::{FlightPlan, Maneuver};
use crate::vectors::Orbit;
use glam::DVec3;
use hifitime::{Epoch, TimeUnits};
use thiserror::Error;
/// PlanErrors can occur from deeper math issues, will get kicked up to the UI from here
#[derive(Debug, Error)]
pub enum PlanError {
    #[error(transparent)]
    Kepler(#[from] KeplerError),
    #[error(transparent)]
    Burn(#[from] BurnError),
    #[error("No convergence when solving, final time: {time} seconds")]
    NotConverged { time: f64 },
    #[error("Invalid acceleration {accel}")]
    InvalidAccel { accel: f64 },
}
/// Planner wraps two Orbits
/// FlightPlans can be generated in different ways from origin to target with methods
#[derive(Debug, Clone, Copy)]
pub struct Planner {
    origin: Orbit,
    target: Orbit,
}
impl Planner {
    pub fn new(origin: Orbit, target: Orbit) -> Self {
        Planner { origin, target }
    }
    /// Returns the brachistochrone FlightPlan from origin to target
    /// while maintaining a fixed acceleration
    /// accel must be finite and nonzero
    pub fn fixed_accel(self, accel: f64) -> Result<FlightPlan, PlanError> {
        if !accel.is_finite() || accel <= 0.0 {
            return Err(PlanError::InvalidAccel { accel });
        }
        let origin_state = self.origin.current_state()?;
        let origin_time = self.origin.epoch();
        let (time, direction) =
            solve_flight_time(self.target, origin_state.position, origin_time, accel)?;
        let leg_time = time / 2.0;
        let maneuvers = vec![
            Maneuver::Burn(Burn::new(origin_time, leg_time, accel, direction)?),
            Maneuver::Burn(Burn::new(
                origin_time + leg_time.seconds(),
                leg_time,
                accel,
                -direction,
            )?),
        ];
        Ok(FlightPlan::new(maneuvers))
    }
}
fn solve_flight_time(
    target: Orbit,
    origin_position: DVec3,
    origin_time: Epoch,
    accel: f64,
) -> Result<(f64, DVec3), PlanError> {
    let target_state = target.current_state()?;
    let distance = target_state.position.distance(origin_position);
    let mut guess_time = 2.0 * (distance.abs() / accel).sqrt();
    const ITERATIONS: u32 = 30;
    const TOLERANCE: f64 = 1.0; // seconds
    for _ in 0..ITERATIONS {
        let target_at_guess = target.state_at(origin_time + guess_time.seconds())?;
        let distance = target_at_guess.position.distance(origin_position);
        let new_time = 2.0 * (distance / accel).sqrt();
        if (new_time - guess_time).abs() <= TOLERANCE {
            return Ok((new_time, target_at_guess.position - origin_position));
        }
        guess_time = new_time;
    }

    Err(PlanError::NotConverged { time: guess_time })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bodies::CentralBody;
    use crate::test_fixtures::{
        emb_orbit, pallas_orbit, stationary_emb, stationary_emb_orbit, stationary_near_origin,
        stationary_near_origin_orbit, test_ship,
    };

    #[test]
    fn invalid_accel() {
        let a_zero = 0.0;
        let a_inf = f64::INFINITY;
        let a_neg = -1.0;
        let a_nan = f64::NAN;
        let planner = Planner::new(emb_orbit(), pallas_orbit());
        let Err(PlanError::InvalidAccel { accel }) = planner.fixed_accel(a_zero) else {
            panic!("Invalid accel {a_zero}")
        };
        assert_eq!(accel, a_zero);
        let Err(PlanError::InvalidAccel { accel }) = planner.fixed_accel(a_inf) else {
            panic!("Invalid accel {a_inf}")
        };
        assert_eq!(accel, a_inf);
        let Err(PlanError::InvalidAccel { accel }) = planner.fixed_accel(a_neg) else {
            panic!("Invalid accel {a_neg}")
        };
        assert_eq!(accel, a_neg);
        let Err(PlanError::InvalidAccel { accel }) = planner.fixed_accel(a_nan) else {
            panic!("Invalid accel {a_nan}")
        };
        assert!(accel.is_nan());
    }

    // Recorded miss: 0.0010911002640611102
    #[test]
    fn stationary_target() {
        let center = CentralBody::None;
        let planner = Planner::new(stationary_emb_orbit(), stationary_near_origin_orbit());
        let test_plan = planner.fixed_accel(33.333).unwrap();
        let current_state = stationary_emb();
        let mut test_ship = test_ship(current_state, center);
        let final_state = test_ship.fly(&test_plan);
        let position_distance = final_state
            .position
            .distance(stationary_near_origin().position);
        let velocity_distance = final_state.velocity.distance(DVec3::ZERO);
        assert!(
            position_distance <= 5e-3,
            "Distance between initial and expected position is {}, greater than tolerance of 5e-3",
            position_distance
        );
        assert!(
            velocity_distance <= 1e-8,
            "Distance between initial and expected velocity is {}, greater than tolerance of 1e-8",
            velocity_distance
        );
    }

    // Recorded miss: 703.2347120924927
    #[test]
    fn moving_target() {
        let center = CentralBody::None;
        let planner = Planner::new(stationary_emb_orbit(), pallas_orbit());
        let test_plan = planner.fixed_accel(33.333).unwrap();
        let current_state = stationary_emb();
        let mut test_ship = test_ship(current_state, center);
        let final_state = test_ship.fly(&test_plan);
        let position_distance = final_state
            .position
            .distance(pallas_orbit().state_at(test_ship.now()).unwrap().position);
        let velocity_distance = final_state.velocity.distance(DVec3::ZERO);
        assert!(
            position_distance <= 2e3,
            "Distance between initial and expected position is {}, greater than tolerance of 2e3",
            position_distance
        );
        assert!(
            velocity_distance <= 1e-8,
            "Distance between initial and expected velocity is {}, greater than tolerance of 1e-8",
            velocity_distance
        );
    }
}
