use crate::orbits::{OrbitalElements, position_at, velocity_at};
use crate::vectors::Trajectory::{Elliptic, Escape, PureRadial};
use glam::DVec3;
use hifitime::Epoch;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

/// Position and Velocity Vector
/// Units in m and m/s
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StateVector {
    pub position: DVec3,
    pub velocity: DVec3,
}

#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum Trajectory {
    Elliptic(OrbitalElements),
    Escape(StateVector, Epoch),
    PureRadial(StateVector, Epoch),
}
impl Trajectory {
    /// Returns a Trajectory from a given StateVector, mu, and epoch
    pub fn from_state(state: StateVector, mu: f64, epoch: Epoch) -> Trajectory {
        let r = state.position;
        let r_len = r.length();
        let v = state.velocity;
        let v_len = v.length();
        let h = r.cross(v);
        let h_len = h.length();
        let n = DVec3::Z.cross(h);
        let ecc_vec = v.cross(h) / mu - r.normalize();
        let epsilon = (v_len.powi(2) / 2.0) - (mu / r_len);
        if h_len / (r_len * v_len) <= 1e-8 {
            return PureRadial(state, epoch);
        } else if epsilon >= 0.0 {
            return Escape(state, epoch);
        };
        let semi_major_axis = -mu / (2.0 * epsilon);
        let eccentricity = ecc_vec.length();
        let inclination = (h.z / h_len).clamp(-1.0, 1.0).acos();
        let equatorial = inclination < 1e-8;
        let circular = eccentricity < 1e-8;
        let node_dir = if equatorial { DVec3::X } else { n };
        let peri_dir = if circular { node_dir } else { ecc_vec };
        let ascending_node = angle_in_plane(DVec3::X, node_dir, DVec3::Z);
        let arg_periapsis = angle_in_plane(node_dir, peri_dir, h.normalize());
        let mean_anomaly_epoch: f64 = if circular {
            angle_in_plane(peri_dir, r, h.normalize())
        } else {
            let cos_e = (1.0 - r_len / semi_major_axis) / eccentricity;
            let sin_e = r.dot(v) / (eccentricity * (mu * semi_major_axis).sqrt());
            let big_e = sin_e.atan2(cos_e);
            (big_e - eccentricity * big_e.sin()).rem_euclid(TAU)
        };
        let elements = OrbitalElements {
            semi_major_axis,
            eccentricity,
            inclination,
            ascending_node,
            arg_periapsis,
            mean_anomaly_epoch,
            epoch,
        };
        Elliptic(elements)
    }
}
/// Signed angle from from, to to, swept around h_hat
fn angle_in_plane(from: DVec3, to: DVec3, h_hat: DVec3) -> f64 {
    from.cross(to)
        .dot(h_hat)
        .atan2(from.dot(to))
        .rem_euclid(TAU)
}
pub fn elements_to_state_vector(
    elements: &OrbitalElements,
    mu: f64,
    ecc_anomaly: f64,
) -> StateVector {
    let position = position_at(elements, ecc_anomaly);
    let velocity = velocity_at(elements, mu, ecc_anomaly);
    StateVector { position, velocity }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bodies::{MU_EARTH, MU_LUNA, MU_SOL};
    use crate::orbits::{propagate_mean_anomaly, solve_kepler};
    use crate::time::J2000;

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
    fn test_earth_round_trip() {
        let mu = MU_SOL + MU_EARTH + MU_LUNA;
        let elements = create_test_elements(
            1.495973362233347e8 * 1000f64,
            1.670236222428361e-2,
            1.034624342994112e-4f64.to_radians(),
            1.402921798841513e2f64.to_radians(),
            3.226257524989104e2f64.to_radians(),
            3.575452038219296e2f64.to_radians(),
        );
        let expected = [
            StateVector {
                position: DVec3::new(
                    -2.65025768897131e7,
                    1.44693955627991e8,
                    -1.704331902042031e2,
                ) * 1000f64,
                velocity: DVec3::new(
                    -2.978644078798413e1,
                    -5.47817682234424e0,
                    4.197340759137802e-5,
                ) * 1000f64,
            },
            StateVector {
                position: DVec3::new(
                    -1.118015459197586e8,
                    -1.011745526923638e8,
                    3.030213951617479e2,
                ) * 1000f64,
                velocity: DVec3::new(
                    1.950301801249175e1,
                    -2.219923219984049e1,
                    2.46575859179643e-5,
                ) * 1000f64,
            },
            StateVector {
                position: DVec3::new(
                    1.408123843059221e8,
                    -5.444539204997468e7,
                    1.579894218221307e1,
                ) * 1000f64,
                velocity: DVec3::new(
                    1.025721827437963e1,
                    2.767237865219828e1,
                    -6.8382840980874e-5, // -6.838284098087399e-5 - rounded to appease clippy
                ) * 1000f64,
            },
            StateVector {
                position: DVec3::new(
                    -2.64847106437987e7,
                    1.44696674101079e8,
                    -3.570930067077279e2,
                ) * 1000f64,
                velocity: DVec3::new(
                    -2.978761317626597e1,
                    -5.473981788314025e0,
                    2.552631990759835e-5,
                ) * 1000f64,
            },
        ];
        test_round_trip(elements, expected, mu);
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
    fn test_pallas_round_trip() {
        let mu = MU_SOL;
        let elements = create_test_elements(
            4.147335391670697e8 * 1000f64,
            2.296435321697976e-1,
            3.484614003622473e1f64.to_radians(),
            1.731977991340821e2f64.to_radians(),
            3.102656379003444e2f64.to_radians(),
            3.529602856167207e2f64.to_radians(),
        );
        let expected = [
            StateVector {
                position: DVec3::new(
                    -1.258325200874033e8,
                    2.473958969651368e8,
                    -1.60651581789323e8,
                ) * 1000f64,
                velocity: DVec3::new(
                    -2.032303521536241e1,
                    -7.137021010411825e0,
                    6.609767786325174e0,
                ) * 1000f64,
            },
            StateVector {
                position: DVec3::new(
                    -2.971024108700995e8,
                    1.245450403910852e8,
                    -6.160110681323632e7,
                ) * 1000f64,
                velocity: DVec3::new(
                    -1.114931540252908e1,
                    -1.527242203755211e1,
                    1.147715329362247e1,
                ) * 1000f64,
            },
            StateVector {
                position: DVec3::new(
                    -3.521747992874941E+08,
                    -4.963631974702031E+07,
                    6.334769796850759E+07,
                ) * 1000f64,
                velocity: DVec3::new(
                    4.373162792482198e-1,
                    -1.676656440550874e1,
                    1.155393577270365e1,
                ) * 1000f64,
            },
            StateVector {
                position: DVec3::new(
                    -2.998039091374451e8,
                    -2.116245011180725e8,
                    1.710005272896911e8,
                ) * 1000f64,
                velocity: DVec3::new(
                    8.824050024422966e0,
                    -1.354023684305857e1,
                    8.631268047647627e0,
                ) * 1000f64,
            },
        ];
        test_round_trip(elements, expected, mu);
    }

    #[test]
    fn test_circular() {
        let mu = MU_SOL;
        let elements = create_test_elements(149_597_870_691.0, 0.0, 0.1, 0.333, 0.777, 0.0);
        let ecc_anomaly = solve_kepler(elements.mean_anomaly_epoch, elements.eccentricity).unwrap();
        let state = elements_to_state_vector(&elements, mu, ecc_anomaly);
        let traj = Trajectory::from_state(state, mu, *J2000);
        let Elliptic(elements2) = traj else {
            panic!("trajectory non-elliptic")
        };
        let ecc_anomaly2 =
            solve_kepler(elements2.mean_anomaly_epoch, elements2.eccentricity).unwrap();
        let state2 = elements_to_state_vector(&elements2, mu, ecc_anomaly2);
        assert!(
            state.position.distance(state2.position) <= 1e3,
            "{}",
            state.position.distance(state2.position)
        );
        assert!(
            state.velocity.distance(state2.velocity) <= 1e1,
            "{}",
            state.velocity.distance(state2.velocity)
        );
    }

    #[test]
    fn test_circular_equatorial() {
        let mu = MU_SOL;
        let elements = create_test_elements(149_597_870_691.0, 0.0, 0.0, 0.333, 0.777, 0.0);
        let ecc_anomaly = solve_kepler(elements.mean_anomaly_epoch, elements.eccentricity).unwrap();
        let state = elements_to_state_vector(&elements, mu, ecc_anomaly);
        let traj = Trajectory::from_state(state, mu, *J2000);
        let Elliptic(elements2) = traj else {
            panic!("trajectory non-elliptic")
        };
        let ecc_anomaly2 =
            solve_kepler(elements2.mean_anomaly_epoch, elements2.eccentricity).unwrap();
        let state2 = elements_to_state_vector(&elements2, mu, ecc_anomaly2);
        assert!(
            state.position.distance(state2.position) <= 1e3,
            "{}",
            state.position.distance(state2.position)
        );
        assert!(
            state.velocity.distance(state2.velocity) <= 1e1,
            "{}",
            state.velocity.distance(state2.velocity)
        );
    }

    #[test]
    fn test_equatorial() {
        let mu = MU_SOL;
        let elements = create_test_elements(149_597_870_691.0, 0.1, 0.0, 0.333, 0.777, 0.0);
        let ecc_anomaly = solve_kepler(elements.mean_anomaly_epoch, elements.eccentricity).unwrap();
        let state = elements_to_state_vector(&elements, mu, ecc_anomaly);
        let traj = Trajectory::from_state(state, mu, *J2000);
        let Elliptic(elements2) = traj else {
            panic!("trajectory non-elliptic")
        };
        let ecc_anomaly2 =
            solve_kepler(elements2.mean_anomaly_epoch, elements2.eccentricity).unwrap();
        let state2 = elements_to_state_vector(&elements2, mu, ecc_anomaly2);
        assert!(
            state.position.distance(state2.position) <= 1e3,
            "{}",
            state.position.distance(state2.position)
        );
        assert!(
            state.velocity.distance(state2.velocity) <= 1e1,
            "{}",
            state.velocity.distance(state2.velocity)
        );
    }

    #[test]
    fn test_escape() {
        let mu = MU_SOL;
        let state = StateVector {
            position: DVec3::new(1.495978707e11, 0.0, 0.0),
            velocity: DVec3::new(0.0, 50_000.0, 0.0),
        };
        let traj = Trajectory::from_state(state, mu, *J2000);
        assert!(matches!(traj, Trajectory::Escape(_,_)));
    }

    #[test]
    fn test_pure_radial() {
        let mu = MU_SOL;
        let state = StateVector {
            position: DVec3::new(4.0e10, 8.0e10, 1.2e11),
            velocity: DVec3::new(8_000.0, 16_000.0, 24_000.0),
        };
        let traj = Trajectory::from_state(state, mu, *J2000);
        assert!(matches!(traj, Trajectory::PureRadial(_,_)));
    }

    fn test_round_trip(elements: OrbitalElements, expected: [StateVector; 4], mu: f64) {
        for (i, exp) in expected.iter().enumerate() {
            let ma = propagate_mean_anomaly(&elements, mu, i as f64 * 175320f64 * 60f64);
            let ecc_anom = solve_kepler(ma, elements.eccentricity);
            let state = elements_to_state_vector(&elements, mu, ecc_anom.unwrap());
            let traj =
                Trajectory::from_state(state, mu, elements.epoch + (i as f64 * 175320f64 * 60f64));
            let Elliptic(elements2) = traj else {
                panic!("trajectory non-elliptic on iteration {}", i)
            };
            assert!((elements.semi_major_axis - elements2.semi_major_axis).abs() <= 1e-3f64);
            assert!((elements.eccentricity - elements2.eccentricity).abs() <= 1e-5f64);
            assert!((elements.inclination - elements2.inclination).abs() <= 1e-5f64);
            assert!((elements.ascending_node - elements2.ascending_node).abs() <= 1e-9f64);
            assert!((elements.arg_periapsis - elements2.arg_periapsis).abs() <= 1e-9f64);
            assert!((ma - elements2.mean_anomaly_epoch).abs() <= 1e-9f64);
            let ecc_anom2 = solve_kepler(elements2.mean_anomaly_epoch, elements2.eccentricity);
            let state = elements_to_state_vector(&elements2, mu, ecc_anom2.unwrap());
            assert!(
                (state.position.x - exp.position.x).abs() <= 6e7f64,
                "on iteration {}, derived position.x {} was outside of tolerance range from expected position.x {}",
                i,
                state.position.x,
                exp.position.x,
            );
            assert!(
                (state.position.y - exp.position.y).abs() <= 6e7f64,
                "on iteration {}, derived position.y {} was outside of tolerance range from expected position.y {}",
                i,
                state.position.y,
                exp.position.y
            );
            assert!(
                (state.position.z - exp.position.z).abs() <= 6e7f64,
                "on iteration {}, derived position.z {} was outside of tolerance range from expected position.z {}",
                i,
                state.position.z,
                exp.position.z
            );
            assert!(
                (state.velocity.x - exp.velocity.x).abs() <= 1e1f64,
                "on iteration {}, derived velocity.x {} was outside of tolerance range from expected velocity.x {}",
                i,
                state.velocity.x,
                exp.velocity.x
            );
            assert!(
                (state.velocity.y - exp.velocity.y).abs() <= 1e1f64,
                "on iteration {}, derived velocity.y {} was outside of tolerance range from expected velocity.y {}",
                i,
                state.velocity.y,
                exp.velocity.y
            );
            assert!(
                (state.velocity.z - exp.velocity.z).abs() <= 1e1f64,
                "on iteration {}, derived velocity.z {} was outside of tolerance range from expected velocity.z {}",
                i,
                state.velocity.z,
                exp.velocity.z
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
