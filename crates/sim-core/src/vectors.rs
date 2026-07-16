use crate::orbits::{OrbitalElements, position_at};
use glam::DVec2;
use hifitime::Epoch;
use serde::{Serialize, Deserialize};

/// Position and Velocity Vector
/// Units in m and m/s
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StateVector {
    pub position: DVec2,
    pub velocity: DVec2,
}

#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum Trajectory {
    Elliptic(OrbitalElements),
    Retrograde(OrbitalElements),
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

pub fn velocity_at(elements: &OrbitalElements, mu: f64, ecc_anomaly: f64) -> DVec2 {
    let r = elements.semi_major_axis * (1.0 - (elements.eccentricity * ecc_anomaly.cos()));
    let n = (mu / elements.semi_major_axis.powi(3)).sqrt();
    let mag = elements.semi_major_axis.powi(2);
    let f = 1.0 - elements.eccentricity.powi(2).sqrt();
    let x_prime = -(n * mag / r) * ecc_anomaly.sin();
    let y_prime = (n * mag / r) - f * ecc_anomaly.cos();
    DVec2::from_angle(elements.arg_periapsis).rotate(DVec2::new(x_prime, y_prime))
}
