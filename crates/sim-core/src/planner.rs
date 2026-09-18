//! Flight Solvers

use crate::vectors::Orbit;

#[derive(Debug, Clone, Copy)]
pub struct Planner {
    origin: Orbit,
    target: Orbit,
}
impl Planner {
    pub fn new(origin: Orbit, target: Orbit) -> Self {
        Planner { origin, target }
    }
}
