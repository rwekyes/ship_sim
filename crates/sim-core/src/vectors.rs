use crate::orbits::{OrbitalElements, position_at, velocity_at};
use crate::vectors::Trajectory::{Elliptic, Escape, PureRadial};
use glam::DVec3;
use hifitime::Epoch;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

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
impl Trajectory {
    pub fn from_state(state: StateVector, mu: f64, epoch: Epoch) -> Trajectory {
        let r = state.position;
        let r_len = r.length();
        let v = state.velocity;
        let v_len = v.length();
        let h = r.cross(v);
        let h_len = h.length();
        let n = DVec3::Z.cross(h);
        let ecc_vec = v.cross(h) / mu - r.normalize();
        let epsilon = (v_len.powi(2) / 2.0) - (mu / r_len);
        if h_len / (r_len * v_len) <= 1e-8 {
            return PureRadial(state, epoch);
        } else if epsilon >= 0.0 {
            return Escape(state, epoch);
        };
        let semi_major_axis = -mu / (2.0 * epsilon);
        let eccentricity = ecc_vec.length();
        let inclination = (h.z / h_len).clamp(-1.0, 1.0).acos();
        let equatorial = inclination < 1e-8;
        let circular = eccentricity < 1e-8;
        let node_dir = if equatorial { DVec3::X } else { n };
        let peri_dir = if circular { node_dir } else { ecc_vec };
        let ascending_node = angle_in_plane(DVec3::X, node_dir, DVec3::Z);
        let arg_periapsis = angle_in_plane(node_dir, peri_dir, h.normalize());
        let mean_anomaly_epoch: f64 = if circular {
            angle_in_plane(peri_dir, r, h.normalize())
        } else {
            let cos_e = (1.0 - r_len / semi_major_axis) / eccentricity;
            let sin_e = r.dot(v) / (eccentricity * (mu * semi_major_axis).sqrt());
            let big_e = sin_e.atan2(cos_e);
            big_e - eccentricity * big_e.sin().rem_euclid(TAU)
        };
        let elements = OrbitalElements {
            semi_major_axis,
            eccentricity,
            inclination,
            ascending_node,
            arg_periapsis,
            mean_anomaly_epoch,
            epoch,
        };
        Elliptic(elements)
    }
}

fn angle_in_plane(from: DVec3, to: DVec3, h_hat: DVec3) -> f64 {
    from.cross(to)
        .dot(h_hat)
        .atan2(from.dot(to))
        .rem_euclid(TAU)
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
