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
    let mut t = t0;

    for i in 0..(n as u32) {
        let a = accel(t, &state);
        state.velocity += a * h;
        state.position += state.velocity * h;
        t = t0 + i as f64 * h;
    }
    state
}

#[cfg(test)]
mod tests {}
