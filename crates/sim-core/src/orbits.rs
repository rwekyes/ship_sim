use glam::{DMat3, DVec3};
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

impl OrbitalElements {
    pub fn position_at_dt(self, mu: f64, dt: f64) -> Result<DVec3, KeplerError> {
        let ma = propagate_mean_anomaly(&self, mu, dt);
        let ecc_anomaly = solve_kepler(ma, self.eccentricity)?;
        Ok(position_at(&self, ecc_anomaly))
    }
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
/// ecc_anomaly is radians from solve_kepler, returns DVec3 in meters in heliocentric frame
pub fn position_at(elements: &OrbitalElements, ecc_anomaly: f64) -> DVec3 {
    let x_prime = elements.semi_major_axis * (ecc_anomaly.cos() - elements.eccentricity);
    let y_prime =
        elements.semi_major_axis * (1.0 - elements.eccentricity.powi(2)).sqrt() * ecc_anomaly.sin();
    let rotation = perifocal_to_ecliptic(elements);
    rotation.mul_vec3(DVec3::new(x_prime, y_prime, 0.0))
}
/// Computes velocity vector in heliocentric frame
/// ecc_anomaly is radians from solve_kepler, returns DVec3 in meters / sec
pub fn velocity_at(elements: &OrbitalElements, mu: f64, ecc_anomaly: f64) -> DVec3 {
    let r = elements.semi_major_axis * (1.0 - (elements.eccentricity * ecc_anomaly.cos()));
    let n = (mu / elements.semi_major_axis.powi(3)).sqrt();
    let ratio = (1.0 - elements.eccentricity.powi(2)).sqrt();
    let n_a_sq_r = n * elements.semi_major_axis.powi(2) / r;
    let x_prime = -n_a_sq_r * ecc_anomaly.sin();
    let y_prime = n_a_sq_r * ratio * ecc_anomaly.cos();
    let rotation = perifocal_to_ecliptic(elements);
    rotation.mul_vec3(DVec3::new(x_prime, y_prime, 0.0))
}
// Order matters to glam, it works right to left - Rz(Ω) * Rx(i) * Rz(ω)
fn perifocal_to_ecliptic(elements: &OrbitalElements) -> DMat3 {
    DMat3::from_rotation_z(elements.ascending_node)
        * DMat3::from_rotation_x(elements.inclination)
        * DMat3::from_rotation_z(elements.arg_periapsis)
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
        // Circular orbit, 1 AU, non-zero omega, inclination, and argument of periapsis
        let elements = create_test_elements(149_597_870_691.0, 0.0, 0.1, 0.333, 0.777, 0.0);
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
            let argument_of_latitude = elements.arg_periapsis + big_e;
            let position2 = DMat3::from_rotation_z(elements.ascending_node)
                * DMat3::from_rotation_x(elements.inclination)
                * (elements.semi_major_axis
                    * DVec3::new(argument_of_latitude.cos(), argument_of_latitude.sin(), 0.0));
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
        let elements = create_test_elements(149_597_870_691.0, 0.3, 0.0, 0.333, 0.0, 0.0);
        let position = position_at(&elements, 0.0);
        assert!((position.length() - 149_597_870_691.0 * (1.0 - 0.3)).abs() <= 1.0);
    }
    #[test]
    fn test_position_at_apoapsis() {
        use std::f64::consts::PI;
        let elements = create_test_elements(149_597_870_691.0, 0.3, 0.0, 0.333, 0.0, 0.0);
        let position = position_at(&elements, PI);
        assert!((position.length() - 149_597_870_691.0 * (1.0 + 0.3)).abs() <= 1.0);
    }
    // Known-answer test against JPL Horizons (DE441), retrieved 2026-07-14.
    // Target: Earth-Moon Barycenter (3), center: Sun body center (500@10),
    // frame: Ecliptic of J2000.0, units KM-S/deg.
    // Span JD 2451545.0..2451910.25 (J2000 + 365.25 d), step 175,320 min.
    // Raw output: test_data/horizons_emb_elements.txt, horizons_emb_vectors.txt.
    // Horizons' "Keplerian GM" = 1.3271284354451501e11 km³/s²,
    //   == MU_SOL + MU_EARTH + MU_LUNA (validates bodies.rs sum).
    // Observed miss vs DE441: 0.15 m at dt=0; ~3,300–7,700 km over the year
    //   (two-body assumption cost; tolerance 1e7 m set with headroom).
    #[test]
    fn test_earth_coasting_to_jpl_data() {
        let elements = OrbitalElements {
            semi_major_axis: 1.495973362233347e8 * 1000f64, // km conversion
            eccentricity: 1.670236222428361e-2,
            inclination: 1.034624342994112e-4f64.to_radians(),
            ascending_node: 1.402921798841513e2f64.to_radians(),
            arg_periapsis: 3.226257524989104e2f64.to_radians(),
            mean_anomaly_epoch: 3.575452038219296e2f64.to_radians(),
            epoch: *J2000,
        };
        let expected = [
            DVec3::new(
                -2.65025768897131e7f64,
                1.44693955627991e8f64,
                -1.704331902042031e2,
            ) * 1e3,
            DVec3::new(
                -1.118015459197586e8,
                -1.011745526923638e8,
                3.030213951617479e2,
            ) * 1e3,
            DVec3::new(
                1.408123843059221e8,
                -5.444539204997468e7,
                1.579894218221307e1,
            ) * 1e3,
            DVec3::new(
                -2.64847106437987e7,
                1.44696674101079e8,
                -3.570930067077279e2,
            ) * 1e3,
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
                "iteration {} distance between calculated {} and expected {} is greater than 10,000 km",
                i,
                position,
                exp
            );
        }
    }
    // Known-answer test against JPL Horizons (JPL#74), retrieved 2026-07-17.
    // Target: 2 Pallas (A802 FA), center: Sun body center (500@10),
    // frame: Ecliptic of J2000.0, units KM-S/deg.
    // Span JD 2451545.0..2451910.25 (J2000 + 365.25 d), step 175,320 min.
    // Raw output: test_data/horizons_pallas_elements.txt, horizons_pallas_vectors.txt.
    // Horizons' "Keplerian GM" = 1.3271244004127939e11 km³/s²,
    //   == MU_SOL
    // Observed miss vs JPL#74: ~6,041 / 22,235 / 43,135 km over the year
    //   (two-body assumption cost; tolerance 6e7 m set with headroom).
    #[test]
    fn test_pallas_coasting_to_jpl_data() {
        let elements = OrbitalElements {
            semi_major_axis: 4.147335391670697e8 * 1000f64, // km conversion
            eccentricity: 2.296435321697976e-1,
            inclination: 3.484614003622473e1f64.to_radians(),
            ascending_node: 1.731977991340821e2f64.to_radians(),
            arg_periapsis: 3.102656379003444e2f64.to_radians(),
            mean_anomaly_epoch: 3.529602856167207e2f64.to_radians(),
            epoch: *J2000,
        };
        let expected = [
            DVec3::new(
                -1.258325200874033e8,
                2.473958969651368e8,
                -1.60651581789323e8,
            ) * 1e3,
            DVec3::new(
                -2.971024108700995e8,
                1.245450403910852e8,
                -6.160110681323632e7,
            ) * 1e3,
            DVec3::new(
                -3.521747992874941e8,
                -4.963631974702031e7,
                6.334769796850759e7,
            ) * 1e3,
            DVec3::new(
                -2.998039091374451e8,
                -2.116245011180725e8,
                1.710005272896911e8,
            ) * 1e3,
        ];
        for (i, exp) in expected.into_iter().enumerate() {
            let ma = propagate_mean_anomaly(&elements, MU_SOL, 175_320f64 * 60f64 * i as f64);
            let big_e = solve_kepler(ma, elements.eccentricity).unwrap();
            let position = position_at(&elements, big_e);
            assert!(
                position.distance(exp) <= 6e7,
                "iteration {} distance between calculated {} and expected {} is greater than 60,000 km",
                i,
                position,
                exp
            );
        }
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
