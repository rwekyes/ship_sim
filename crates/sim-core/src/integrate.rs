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
    use crate::integrate::two_body;
    use crate::orbits::{OrbitalElements, solve_kepler};
    use crate::time::J2000;
    use crate::vectors::elements_to_state_vector;
    use std::f64::consts::TAU;

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
