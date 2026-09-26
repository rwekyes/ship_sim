//! Helper functions for tests in sim-core

use crate::bodies::CentralBody;
use crate::orbits::{OrbitalElements, solve_kepler};
use crate::ship::Ship;
use crate::time::{Clock, J2000};
use crate::vectors::{Orbit, StateVector, elements_to_state_vector};
use glam::DVec3;

pub fn test_ship(current_state: StateVector, center: CentralBody) -> Ship {
    let name = String::from("Test Ship");
    let clock = Clock::new(*J2000);
    let transponder_id = String::from("123456789");
    let mass = 1.0;
    let max_accel = 1000.0;
    Ship::new(
        name,
        current_state,
        center,
        clock,
        transponder_id,
        mass,
        max_accel,
    )
}
// Not Horizons data - only for unit tests
pub fn emb_orbit() -> Orbit {
    let elements = emb_elements();
    let state = elements_to_state_vector(
        &elements,
        CentralBody::Sol.mu(),
        solve_kepler(elements.mean_anomaly_epoch, elements.eccentricity).unwrap(),
    );
    Orbit::from_state(state, CentralBody::Sol, *J2000)
}

pub fn pallas_orbit() -> Orbit {
    let elements = pallas_elements();
    let state = elements_to_state_vector(
        &elements,
        CentralBody::Sol.mu(),
        solve_kepler(elements.mean_anomaly_epoch, elements.eccentricity).unwrap(),
    );
    Orbit::from_state(state, CentralBody::Sol, *J2000)
}

pub fn stationary_emb_orbit() -> Orbit {
    Orbit::from_state(stationary_emb(), CentralBody::FlatSpace, *J2000)
}

pub fn stationary_near_origin_orbit() -> Orbit {
    Orbit::from_state(stationary_near_origin(), CentralBody::FlatSpace, *J2000)
}

pub fn stationary_near_origin() -> StateVector {
    StateVector {
        position: DVec3::new(1.0, 1.0, 1.0),
        velocity: DVec3::ZERO,
    }
}

pub fn stationary_emb() -> StateVector {
    StateVector {
        position: DVec3::new(
            -2.65025768897131e7,
            1.44693955627991e8,
            -1.704331902042031e2,
        ) * 1.0e3,
        velocity: DVec3::ZERO,
    }
}

pub fn emb_elements() -> OrbitalElements {
    create_test_elements(
        1.495973362233347e8 * 1000f64,
        1.670236222428361e-2,
        1.034624342994112e-4f64.to_radians(),
        1.402921798841513e2f64.to_radians(),
        3.226257524989104e2f64.to_radians(),
        3.575452038219296e2f64.to_radians(),
    )
}

pub fn pallas_elements() -> OrbitalElements {
    create_test_elements(
        4.147335391670697e8 * 1000f64,
        2.296435321697976e-1,
        3.484614003622473e1f64.to_radians(),
        1.731977991340821e2f64.to_radians(),
        3.102656379003444e2f64.to_radians(),
        3.529602856167207e2f64.to_radians(),
    )
}
pub fn create_test_elements(
    semi_major_axis: f64,
    eccentricity: f64,
    inclination: f64,
    ascending_node: f64,
    arg_periapsis: f64,
    mean_anomaly_epoch: f64,
) -> OrbitalElements {
    OrbitalElements {
        semi_major_axis,
        eccentricity,
        inclination,
        ascending_node,
        arg_periapsis,
        mean_anomaly_epoch,
        epoch: *J2000,
    }
}
