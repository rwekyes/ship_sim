use hifitime::Epoch;
use serde::{Deserialize, Serialize};
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
    /// Argument of periapsis
    pub arg_periapsis: f64,
    /// Mean anomaly at epoch (rad)
    pub mean_anomaly_epoch: f64,
    /// Reference time at which mean_anomaly_epoch applies
    pub epoch: Epoch,
}
