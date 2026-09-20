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
