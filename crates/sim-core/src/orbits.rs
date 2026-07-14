use hifitime::Epoch;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;
use thiserror::Error;
/// Error types for Kepler solver
#[derive(Debug, Error)]
pub enum KeplerError {
    #[error("Eccentricity {0} is >= 1")]
    NotElliptical(f64),
    #[error("No convergence after {iters} iterations (M={mean_anomaly}, e={eccentricity})")]
    NotConverged {
        iters: u32,
        mean_anomaly: f64,
        eccentricity: f64,
    },
}
/// Orbital Elements struct
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrbitalElements {
    /// Semi-major axis (m)
    pub semi_major_axis: f64,
    /// Eccentricity (dimensionless)
    pub eccentricity: f64,
    /// Inclination (rad)
    pub inclination: f64,
    /// Longitude of ascending node (rad)
    pub ascending_node: f64,
    /// Argument of periapsis (rad)
    pub arg_periapsis: f64,
    /// Mean anomaly at epoch (rad)
    pub mean_anomaly_epoch: f64,
    /// Reference time at which mean_anomaly_epoch applies
    pub epoch: Epoch,
}
/// Propagate the mean anomaly
///
/// μ is m³/s², a³ is m³, so μ/a³ is 1/s² and its square root is rad/s.
/// mu can be retrieved from bodies.rs per major body or supplied raw
/// dt is seconds since elements.epoch
pub fn propagate_mean_anomaly(elements: &OrbitalElements, mu: f64, dt: f64) -> f64 {
    (((mu / elements.semi_major_axis.powi(3)).sqrt() * dt) + elements.mean_anomaly_epoch)
        .rem_euclid(TAU)
}
/// Kepler solver
///
/// Solves M = E - e*sin(E) for the eccentric anomaly
/// Inputs in radians, ma is assumed to be wrapped in [0,2π)
/// after calculating with propagate_mean_anomaly
pub fn solve_kepler(ma: f64, ecc: f64) -> Result<f64, KeplerError> {
    if ecc >= 1.0 {
        return Err(KeplerError::NotElliptical(ecc));
    }
    const MAX_ITERS: u32 = 30;
    const TOLERANCE: f64 = 1e-12;

    let mut e_anom = ma;

    for _ in 0..MAX_ITERS {
        let f = e_anom - (ecc * e_anom.sin()) - ma;
        let f_slope = 1.0 - (ecc * e_anom.cos());
        let delta = f / f_slope;
        e_anom -= delta;
        if delta.abs() < TOLERANCE {
            return Ok(e_anom);
        }
    }

    Err(KeplerError::NotConverged {
        iters: MAX_ITERS,
        mean_anomaly: ma,
        eccentricity: ecc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solve_kepler_zero_ma() {
        assert_eq!(solve_kepler(0.0, 0.5).unwrap(), 0.0);
    }

    #[test]
    fn test_solve_kepler_zero_ecc() {
        assert_eq!(solve_kepler(0.5, 0.0).unwrap(), 0.5);
    }

    #[test]
    fn test_solve_kepler_pairs_to_tolerance() {
        for i in 0..43 {
            let ma = i as f64 / 43.0;
            for j in 0..13 {
                let ecc = j as f64 / 13.0;
                let big_e = solve_kepler(ma, ecc).unwrap();
                let residual = (big_e - (ecc * big_e.sin()) - ma).abs();
                assert!(
                    residual <= 1e-12,
                    "residual {} is too big for M={}, e={}",
                    residual,
                    ma,
                    ecc
                );
            }
        }
    }
}
