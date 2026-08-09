use crate::vectors::StateVector;
use glam::DVec3;
/// Two body gravitational acceleration calculator
///
/// debug_assert to catch bad vectors in dev
pub fn two_body(mu: f64, state: &StateVector) -> DVec3 {
    let r = state.position;
    debug_assert!(r.is_finite() && r.length() > 1.0, "two_body: bad position {r:?}");
    -mu * r / r.length().powi(3)
}
/// StateVector integrator
///
/// t0 is seconds since the element's epoch
pub fn integrate<F>(
    state: StateVector,
    t0: f64,
    dt_total: f64,
    substep: f64,
    accel: F,
) -> StateVector
where
    F: Fn(f64, &StateVector) -> DVec3,
{
}

#[cfg(test)]
mod tests {}
