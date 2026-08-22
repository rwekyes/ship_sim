use crate::vectors::StateVector;
use glam::DVec3;
/// Two body gravitational acceleration calculator
///
/// debug_assert to catch bad vectors in dev
pub fn two_body(mu: f64, state: &StateVector) -> DVec3 {
    let r = state.position;
    debug_assert!(
        r.is_finite() && r.length() > 1.0,
        "two_body: bad position {r:?}"
    );
    -mu * r / r.length().powi(3)
}
/// StateVector integrator
///
/// state is object's current position and velocity vectors
/// t0 is seconds since the element's epoch
/// dt_total is the advance span, in seconds
/// substep is the maximum size of the integration steps, in seconds
/// accel is the acceleration vector as a closure that returns a DVec3
pub fn integrate<F>(
    mut state: StateVector,
    t0: f64,
    dt_total: f64,
    substep: f64,
    accel: F,
) -> StateVector
where
    F: Fn(f64, &StateVector) -> DVec3,
{
    debug_assert!(
        dt_total >= 0.0 && substep > 0.0,
        "integrate: bad span dt_total={} substep={}",
        dt_total,
        substep
    );
    let n = (dt_total / substep).ceil();
    if n == 0.0 {
        return state;
    }
    let h = dt_total / n;

    for i in 0..(n as u32) {
        let t = t0 + i as f64 * h;
        let a = accel(t, &state);
        state.velocity += a * h;
        state.position += state.velocity * h;
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bodies::{MU_EARTH, MU_LUNA, MU_SOL};
    use crate::burns::Burn;
    use crate::integrate::two_body;
    use crate::orbits::{OrbitalElements, solve_kepler};
    use crate::time::J2000;
    use crate::vectors::elements_to_state_vector;
    use hifitime::TimeUnits;

    // Test the integrator vs the Kepler propagation
    // Observed miss: 2861969.4202166707m in 1/4 orbit for Earth, tolerance set to 3000000m
    #[test]
    fn integrate_vs_kepler() {
        let position_difference = kepler_miss(60.0);
        assert!(
            position_difference < 3e6,
            "Difference between expected position and derived position is {}, which is greater than 3.0e6",
            position_difference
        );
    }
    // Known-answer test against JPL Horizons (DE441), retrieved 2026-07-14.
    // Target: Earth-Moon Barycenter (3), center: Sun body center (500@10),
    // frame: Ecliptic of J2000.0, units KM-S/deg.
    // Span JD 2451545.0..2451910.25 (J2000 + 365.25 d), step 175,320 min.
    // Raw output: test_data/horizons_emb_elements.txt, horizons_emb_vectors.txt.
    // Horizons' "Keplerian GM" = 1.3271284354451501e11 km³/s²,
    //   == MU_SOL + MU_EARTH + MU_LUNA (validates bodies.rs sum).
    // Observed miss vs DE441: 481882.78918451606m, tolerance set to 500000m
    #[test]
    fn coast_vs_horizons() {
        let initial_state: StateVector = StateVector {
            position: DVec3::new(
                -2.65025768897131e7,
                1.44693955627991e8,
                -1.704331902042031e2,
            ) * 1e3,
            velocity: DVec3::new(
                -2.978644078798413e1,
                -5.47817682234424e0,
                4.197340759137802e-5,
            ) * 1e3,
        };
        let expected_state: StateVector = StateVector {
            position: DVec3::new(
                -1.118015459197586e8,
                -1.011745526923638e8,
                3.030213951617479e2,
            ) * 1e3,
            velocity: DVec3::new(
                1.950301801249175e1,
                -2.219923219984049e1,
                2.46575859179643e-5,
            ) * 1e3,
        };
        let new_state = integrate(initial_state, 0.0, 175320.0 * 60.0, 60.0, |_t, s| {
            two_body(MU_SOL + MU_EARTH + MU_LUNA, s)
        });
        let difference: f64 = expected_state.position.distance(new_state.position);
        assert!(
            difference < 5e5,
            "Difference between expected position {} and derived position {} is {}, which is greater than 5.0e5",
            expected_state.position,
            new_state.position,
            difference
        );
    }
    // Step size convergence test
    // Observed miss: 1430987.6428220582m - true half of 2861969.4202166707m is 1430984.710108335m
    // Difference of 2.932713723m, ratio 0.5000010
    #[test]
    fn step_size_convergence() {
        let ratio = kepler_miss(30.0) / kepler_miss(60.0);
        assert!(
            (0.48..=0.52).contains(&ratio),
            "step_size_convergence: ratio {}",
            ratio
        );
    }
    // Observed drift at 100 orbits sampled 20 times per orbit: 0.00001279972527773939
    // Swapping velocity and position updates caused drift: 0.3535623174804111
    #[test]
    fn energy_bound() {
        let mu = MU_SOL + MU_EARTH + MU_LUNA;
        let elements = emb_j2000_elements();
        let ecc_anomaly = solve_kepler(elements.mean_anomaly_epoch, elements.eccentricity).unwrap();
        let mut state = elements_to_state_vector(&elements, mu, ecc_anomaly);
        let epsilon_0 = specific_energy(mu, &state);
        let mut max: f64 = 0.0;
        let mut t0: f64 = 0.0;
        let dt = elements.period(mu) / 20.0;
        for _ in 0..2000 {
            state = integrate(state, t0, dt, 3600.0, |_t, s| two_body(mu, s));
            t0 += dt;
            let epsilon = specific_energy(mu, &state);
            let difference = epsilon - epsilon_0;
            let drift = difference.abs() / epsilon_0.abs();
            if drift > max {
                max = drift;
            }
        }
        assert!(max < 2e-5, "max {} above threshold 2e-5", max)
    }
    // Pure acceleration, mu = 0
    // For a = 0.33, -2.1, 7.7, dt = 333, substep = 13
    // observed residual  = 3.0517578125222045e-05
    // ULP at y=1.4469e11 = 3.0517578125e-05
    #[test]
    fn pure_kinematics() {
        let a = DVec3::new(0.33, -2.1, 7.7);
        let state = StateVector {
            position: DVec3::new(
                -2.65025768897131e7,
                1.44693955627991e8,
                -1.704331902042031e2,
            ) * 1e3,
            velocity: DVec3::new(0.2, 9.9, 5.3),
        };
        let dt: f64 = 333.0;
        let substep = 13.0;
        let n = (dt / substep).ceil();
        let h = dt / n;
        let final_state = integrate(state, 0.0, dt, substep, |_t, _s| a);
        let expected_position =
            state.position + state.velocity * dt + 0.5 * a * dt.powi(2) + 0.5 * a * h * dt;
        let residual = final_state.position.distance(expected_position);
        assert!(
            residual < 1e-4,
            "residual {} is above tolerance 1e-4",
            residual
        );
        let velocity_difference = final_state.velocity.distance(state.velocity + a * dt);
        assert!(
            velocity_difference < 1e-9,
            "velocity difference {} is above tolerance 1e-9",
            velocity_difference
        );
    }
    // Verifies adding a 0 thrust burn in the closure does not change the result
    #[test]
    fn full_closure_with_accel_at() {
        let mu = MU_SOL + MU_EARTH + MU_LUNA;
        let state = StateVector {
            position: DVec3::new(
                -2.65025768897131e7,
                1.44693955627991e8,
                -1.704331902042031e2,
            ) * 1e3,
            velocity: DVec3::new(0.2, 9.9, 5.3),
        };
        let reference = *J2000;
        let dt: f64 = 333.0;
        let substep = 13.0;
        let zero_burn = Burn::new(*J2000, 60.0, 0.0, DVec3::new(0.11, -4.2, 0.77777)).unwrap();
        let bare = integrate(state, 0.0, dt, substep, |_t, s| two_body(mu, s));
        let summed = integrate(state, 0.0, dt, substep, |t, s| {
            two_body(mu, s) + zero_burn.accel_at(reference, t)
        });
        assert_eq!(bare.position, summed.position);
        assert_eq!(bare.velocity, summed.velocity);
    }
    // Burn that fires outside the integrator doesn't affect anything
    #[test]
    fn window_outside_span() {
        let mu = MU_SOL + MU_EARTH + MU_LUNA;
        let state = StateVector {
            position: DVec3::new(
                -2.65025768897131e7,
                1.44693955627991e8,
                -1.704331902042031e2,
            ) * 1e3,
            velocity: DVec3::new(0.2, 9.9, 5.3),
        };
        let reference = *J2000;
        let dt: f64 = 333.0;
        let substep = 13.0;
        let late_burn = Burn::new(
            *J2000 + 334.seconds(),
            60.0,
            3.2675,
            DVec3::new(0.11, -4.2, 0.77777),
        )
        .unwrap();
        let zero_burn = Burn::new(*J2000, 60.0, 0.0, DVec3::new(0.11, -4.2, 0.77777)).unwrap();
        let late_integration = integrate(state, 0.0, dt, substep, |t, s| {
            two_body(mu, s) + late_burn.accel_at(reference, t)
        });
        let zero_integration = integrate(state, 0.0, dt, substep, |t, s| {
            two_body(mu, s) + zero_burn.accel_at(reference, t)
        });
        assert_eq!(late_integration.position, zero_integration.position);
        assert_eq!(late_integration.velocity, zero_integration.velocity);
    }
    // Constant acceleration for the whole span
    #[test]
    fn mu_zero_whole_span() {
        let a = DVec3::new(0.33, -2.1, 7.7);
        let state = StateVector {
            position: DVec3::new(
                -2.65025768897131e7,
                1.44693955627991e8,
                -1.704331902042031e2,
            ) * 1e3,
            velocity: DVec3::new(0.2, 9.9, 5.3),
        };
        let dt: f64 = 999.0;
        let substep = 9.0;
        let n = (dt / substep).ceil();
        let h = dt / n;
        let burn = Burn::new(*J2000, dt, a.length(), a).unwrap();
        let final_state = integrate(state, 0.0, dt, substep, |t, _s| burn.accel_at(*J2000, t));
        let expected_position =
            state.position + state.velocity * dt + 0.5 * a * dt.powi(2) + 0.5 * a * h * dt;
        let residual = final_state.position.distance(expected_position);
        assert!(
            residual < 1e-4,
            "residual {} is above tolerance 1e-4",
            residual
        );
        let velocity_difference = final_state.velocity.distance(state.velocity + a * dt);
        assert!(
            velocity_difference < 1e-9,
            "velocity difference {} is above tolerance 1e-9",
            velocity_difference
        );
    }
    // Coast, burn, coast
    #[test]
    fn mu_zero_partial_span() {
        let a = DVec3::new(0.33, -2.1, 7.7);
        let state = StateVector {
            position: DVec3::new(
                -2.65025768897131e7,
                1.44693955627991e8,
                -1.704331902042031e2,
            ) * 1e3,
            velocity: DVec3::new(0.2, 9.9, 5.3),
        };
        let dt: f64 = 999.0;
        let substep = 9.0;
        let n = (dt / substep).ceil();
        let h = dt / n;
        let burn = Burn::new(*J2000 + 270.seconds(), 360.0, a.length(), a).unwrap();
        let final_state = integrate(state, 0.0, dt, substep, |t, _s| burn.accel_at(*J2000, t));
        let t1 = 270.0;
        let t2 = 360.0;
        let t3 = 369.0;
        let r1 = state.position + state.velocity * t1;
        let v1 = state.velocity;
        let r2 = r1 + v1 * t2 + (a / 2.0) * t2.powi(2) + (a / 2.0) * h * t2;
        let v2 = v1 + a * t2;
        let r3 = r2 + v2 * t3;
        let v3 = v2;
        let residual = final_state.position.distance(r3);
        assert!(
            residual < 1e-3,
            "residual {} is above tolerance 1e-3",
            residual
        );
        let velocity_difference = final_state.velocity.distance(v3);
        assert!(
            velocity_difference < 1e-9,
            "velocity difference {} is above tolerance 1e-9",
            velocity_difference
        );
    }
    // Helper returns J2000 elements for EMB
    fn emb_j2000_elements() -> OrbitalElements {
        OrbitalElements {
            semi_major_axis: 1.495973362233347e8 * 1000f64, // km conversion
            eccentricity: 1.670236222428361e-2,
            inclination: 1.034624342994112e-4f64.to_radians(),
            ascending_node: 1.402921798841513e2f64.to_radians(),
            arg_periapsis: 3.226257524989104e2f64.to_radians(),
            mean_anomaly_epoch: 3.575452038219296e2f64.to_radians(),
            epoch: *J2000,
        }
    }
    // helper computes the miss in meters between the integrator and the keplerian solver
    fn kepler_miss(substep: f64) -> f64 {
        let mu = MU_SOL + MU_EARTH + MU_LUNA;
        let elements = emb_j2000_elements();
        let ecc_anomaly = solve_kepler(elements.mean_anomaly_epoch, elements.eccentricity).unwrap();
        let initial_state = elements_to_state_vector(&elements, mu, ecc_anomaly);
        let new_state = integrate(initial_state, 0.0, 175320.0 * 60.0, substep, |_t, s| {
            two_body(mu, s)
        });

        let position = elements.position_at_dt(mu, 175320.0 * 60.0).unwrap();
        new_state.position.distance(position)
    }
    // helper computes specific energy
    fn specific_energy(mu: f64, state: &StateVector) -> f64 {
        let r = state.position;
        let r_len = r.length();
        let v = state.velocity;
        let v_len_sqr = v.length_squared();
        (v_len_sqr / 2.0) - (mu / r_len)
    }
}
