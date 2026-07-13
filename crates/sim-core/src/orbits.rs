use hifitime::Epoch;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;
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
/// Propagate the mean anomaly
///
/// μ is m³/s², a³ is m³, so μ/a³ is 1/s² and its square root is rad/s.
/// mu can be retrieved from bodies.rs per major body or supplied raw
/// dt is seconds since elements.epoch
pub fn propagate_mean_anomaly(elements: &OrbitalElements, mu: f64, dt: f64) -> f64 {
    (((mu / elements.semi_major_axis.powi(3)).sqrt() * dt) + elements.mean_anomaly_epoch)
        .rem_euclid(TAU)
}
