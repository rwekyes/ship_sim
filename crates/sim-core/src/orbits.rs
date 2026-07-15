use glam::DVec2;
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

/// Computes heliocentric position
/// ecc_anomaly is radians from solve_kepler, returns DVec2 in meters in heliocentric frame
pub fn position_at(elements: &OrbitalElements, ecc_anomaly: f64) -> DVec2 {
    DVec2::from_angle(elements.arg_periapsis).rotate(DVec2::new(
        elements.semi_major_axis * (ecc_anomaly.cos() - elements.eccentricity),
        elements.semi_major_axis * (1.0 - elements.eccentricity.powi(2)).sqrt() * ecc_anomaly.sin(),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bodies::{MU_EARTH, MU_LUNA, MU_SOL};
    use crate::time::J2000;

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
            let ma = (i as f64 / 43.0) * TAU;
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
    #[test]
    fn test_position_at_circle() {
        // Circular orbit, 1 AU, non-zero omega
        let elements = create_test_elements(149_597_870_691.0, 0.0, 0.333);
        for i in 0..23 {
            let ma = (i as f64 / 23.0) * TAU;
            let big_e = solve_kepler(ma, elements.eccentricity).unwrap();
            let position = position_at(&elements, big_e);
            let residual = (position.length() - elements.semi_major_axis).abs();
            assert!(
                residual <= 1.0,
                "on iteration {}, residual {} is larger than 1m",
                i,
                residual
            );
            let position2 =
                elements.semi_major_axis * DVec2::from_angle(elements.arg_periapsis + big_e);
            assert!(
                position.distance(position2) <= 1.0,
                "on iteration {}, vector {} is not within 1 m of vector {}",
                i,
                position,
                position2
            )
        }
    }
    #[test]
    fn test_position_at_periapsis() {
        let elements = create_test_elements(149_597_870_691.0, 0.3, 0.333);
        let position = position_at(&elements, 0.0);
        assert!((position.length() - 149_597_870_691.0 * (1.0 - 0.3)).abs() <= 1.0);
    }
    #[test]
    fn test_position_at_apoapsis() {
        use std::f64::consts::PI;
        let elements = create_test_elements(149_597_870_691.0, 0.3, 0.333);
        let position = position_at(&elements, PI);
        assert!((position.length() - 149_597_870_691.0 * (1.0 + 0.3)).abs() <= 1.0);
    }
    // Known-answer test against JPL Horizons (DE441), retrieved 2026-07-14.
    // Target: Earth-Moon Barycenter (3), center: Sun body center (500@10),
    // frame: Ecliptic of J2000.0, units KM-S/deg.
    // Span JD 2451545.0..2451910.25 (J2000 + 365.25 d), step 175,320 min.
    // Raw output: testdata/horizons_emb_elements.txt, horizons_emb_vectors.txt.
    // Horizons' "Keplerian GM" = 1.3271284354451501e11 km³/s²,
    //   == MU_SOL + MU_EARTH + MU_LUNA (validates bodies.rs sum).
    // Observed miss vs DE441: 0.15 m at dt=0; ~3,300–7,700 km over the year
    //   (two-body assumption cost; tolerance 1e7 m set with headroom).
    #[test]
    fn test_earth_coasting_to_jpl_data_2d() {
        let elements = OrbitalElements {
            semi_major_axis: 1.495973362233347e8 * 1000f64, // km conversion
            eccentricity: 1.670236222428361e-2,
            inclination: 0.0,
            ascending_node: 0.0,
            arg_periapsis: (1.402921798841513e2f64 + 3.226257524989104e2f64).to_radians(),
            mean_anomaly_epoch: 3.575452038219296e2f64.to_radians(),
            epoch: *J2000,
        };
        let expected = [
            DVec2::new(-2.65025768897131e7f64, 1.44693955627991e8f64) * 1e3,
            DVec2::new(-1.118015459197586e8, -1.011745526923638e8) * 1e3,
            DVec2::new(1.408123843059221e8, -5.444539204997468e7) * 1e3,
            DVec2::new(-2.64847106437987e7, 1.44696674101079e8) * 1e3,
        ];
        for (i, exp) in expected.into_iter().enumerate() {
            let ma = propagate_mean_anomaly(
                &elements,
                MU_SOL + MU_EARTH + MU_LUNA,
                175_320f64 * 60f64 * i as f64,
            );
            let big_e = solve_kepler(ma, elements.eccentricity).unwrap();
            let position = position_at(&elements, big_e);
            assert!(
                position.distance(exp) <= 1e7,
                "iteration {} distance between calculated {} and expected {} is greater that 10,000 km",
                i,
                position,
                exp
            );
        }
    }

    fn create_test_elements(
        semi_major_axis: f64,
        eccentricity: f64,
        arg_periapsis: f64,
    ) -> OrbitalElements {
        OrbitalElements {
            semi_major_axis,
            eccentricity,
            inclination: 0.0,
            ascending_node: 0.0,
            arg_periapsis,
            mean_anomaly_epoch: 0.0,
            epoch: *J2000,
        }
    }
}
