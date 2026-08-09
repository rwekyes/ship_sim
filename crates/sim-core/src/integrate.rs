use crate::vectors::StateVector;
use glam::DVec3;

pub fn two_body(mu: f64, state: &StateVector) -> DVec3 {
    let r = state.position;
    -mu * r / r.length().powi(3)
}
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
