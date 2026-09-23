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
        if !accel.is_finite() || accel <= 0.0 || accel.is_nan() {
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
    use crate::orbits::{OrbitalElements, solve_kepler};
    use crate::time::J2000;
    use crate::vectors::{Orbit, elements_to_state_vector};

    #[test]
    fn invalid_accel() {
        let a_zero = 0.0;
        let a_inf = f64::INFINITY;
        let a_neg = -1.0;
        let a_nan = f64::NAN;
        let planner = Planner::new(test_orbit_one(), test_orbit_two());
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
        assert!(a_nan.is_nan());
    }

    // Earth's orbit
    fn test_orbit_one() -> Orbit {
        let elements = create_test_elements(
            1.495973362233347e8 * 1000f64,
            1.670236222428361e-2,
            1.034624342994112e-4f64.to_radians(),
            1.402921798841513e2f64.to_radians(),
            3.226257524989104e2f64.to_radians(),
            3.575452038219296e2f64.to_radians(),
        );
        let state = elements_to_state_vector(
            &elements,
            CentralBody::Sol.mu(),
            solve_kepler(elements.mean_anomaly_epoch, elements.eccentricity).unwrap(),
        );
        Orbit::from_state(state, CentralBody::Sol, *J2000)
    }
    // Pallas' orbit
    fn test_orbit_two() -> Orbit {
        let elements = create_test_elements(
            4.147335391670697e8 * 1000f64,
            2.296435321697976e-1,
            3.484614003622473e1f64.to_radians(),
            1.731977991340821e2f64.to_radians(),
            3.102656379003444e2f64.to_radians(),
            3.529602856167207e2f64.to_radians(),
        );
        let state = elements_to_state_vector(
            &elements,
            CentralBody::Sol.mu(),
            solve_kepler(elements.mean_anomaly_epoch, elements.eccentricity).unwrap(),
        );
        Orbit::from_state(state, CentralBody::Sol, *J2000)
    }

    fn create_test_elements(
        semi_major_axis: f64,
        eccentricity: f64,
        inclination: f64,
        ascending_node: f64,
        arg_periapsis: f64,
        mean_anomaly_epoch: f64,
    ) -> OrbitalElements {
        OrbitalElements {
            semi_major_axis,
            eccentricity,
            inclination,
            ascending_node,
            arg_periapsis,
            mean_anomaly_epoch,
            epoch: *J2000,
        }
    }
}
