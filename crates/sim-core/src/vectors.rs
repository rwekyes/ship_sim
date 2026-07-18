use crate::orbits::{OrbitalElements, position_at, velocity_at};
use glam::DVec3;
use hifitime::Epoch;
use serde::{Deserialize, Serialize};

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

pub fn elements_to_state_vector(
    elements: &OrbitalElements,
    mu: f64,
    ecc_anomaly: f64,
) -> StateVector {
    let position = position_at(elements, ecc_anomaly);
    let velocity = velocity_at(elements, mu, ecc_anomaly);
    StateVector { position, velocity }
}
