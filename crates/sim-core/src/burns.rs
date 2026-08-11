//! Burn related data structures and logic
//! Boundary between Epoch and seconds

use crate::time::seconds_since;
use glam::DVec3;
use hifitime::Epoch;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BurnError {
    #[error("Direction {0} must be a nonzero, finite vector")]
    InvalidDirection(DVec3),
    #[error("Duration {0} must be finite and non-negative")]
    InvalidDuration(f64),
    #[error("Acceleration {0} must be finite and non-negative")]
    InvalidAccel(f64),
}

/// Burn struct
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "BurnRepr")]
pub struct Burn {
    /// Start of burn
    start: Epoch,
    /// Duration of burn (seconds)
    duration: f64,
    /// Acceleration (meters per second squared)
    accel: f64,
    /// Direction - unit vector, fixed inertial heading, normalized
    direction: DVec3,
}

impl Burn {
    /// Constructor returns a BurnError if any parameters are negative, non-finite, or if direction cannot be normalized.
    pub fn new(
        start: Epoch,
        duration: f64,
        accel: f64,
        direction: DVec3,
    ) -> Result<Self, BurnError> {
        let direction = direction
            .try_normalize()
            .ok_or(BurnError::InvalidDirection(direction))?;
        if !duration.is_finite() || duration < 0.0 {
            return Err(BurnError::InvalidDuration(duration));
        } else if !accel.is_finite() || accel < 0.0 {
            return Err(BurnError::InvalidAccel(accel));
        }
        Ok(Self {
            start,
            duration,
            accel,
            direction,
        })
    }
    /// Thrust acceleration from this burn at time t, in meters per second squared
    pub fn accel_at(self, reference: Epoch, t: f64) -> DVec3 {
        debug_assert!(t.is_finite());
        let start_s = seconds_since(reference, self.start);
        let end_s = start_s + self.duration;
        if (start_s..end_s).contains(&t) {
            self.accel * self.direction
        } else {
            DVec3::ZERO
        }
    }

    pub fn start(self) -> Epoch {
        self.start
    }

    pub fn duration(self) -> f64 {
        self.duration
    }

    pub fn accel(self) -> f64 {
        self.accel
    }

    pub fn direction(self) -> DVec3 {
        self.direction
    }
}

#[derive(Deserialize)]
struct BurnRepr {
    start: Epoch,
    duration: f64,
    accel: f64,
    direction: DVec3,
}

impl TryFrom<BurnRepr> for Burn {
    type Error = BurnError;
    fn try_from(repr: BurnRepr) -> Result<Self, Self::Error> {
        Burn::new(repr.start, repr.duration, repr.accel, repr.direction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::J2000;
    use hifitime::TimeUnits;
    #[test]
    fn serialize_round_trip() {
        let burn = Burn::new(*J2000, 60.0, 100.0, DVec3::new(0.11, -4.2, 0.77777)).unwrap();
        let string = serde_json::to_string(&burn).unwrap();
        let new_burn: Burn = serde_json::from_str(&string).unwrap();
        assert_eq!(burn.start(), new_burn.start());
        assert_eq!(burn.duration(), new_burn.duration());
        assert_eq!(burn.accel(), new_burn.accel());
        assert!(
            burn.direction().distance(new_burn.direction()) < 1e-15,
            "burn direction difference is {}, greater than tolerance 1e-15",
            burn.direction().distance(new_burn.direction())
        );
    }
    // Makes sure normalization happens, commenting out the try_from call causes failure
    #[test]
    fn non_unit_json() {
        let json = r#"{"start":"2000-01-01T12:00:00 TT","duration":60.0,"accel":100.0,"direction":[2.0,-3.0,6.0]}"#;
        let burn: Burn = serde_json::from_str(json).unwrap();
        assert!(burn.direction().distance(DVec3::new(2.0, -3.0, 6.0) / 7.0) < 1e-15);
    }
    // Correct error shape for zero, nan, and infinity
    #[test]
    fn invalid_direction() {
        let zero_vec = DVec3::new(0.0, 0.0, 0.0);
        let infinite_vec = DVec3::new(f64::INFINITY, 0.0, 0.0);
        let nan_vec = DVec3::new(f64::NAN, 0.0, 0.0);
        let Err(BurnError::InvalidDirection(v)) = Burn::new(*J2000, 60.0, 100.0, zero_vec) else {
            panic!("Expected an invalid direction");
        };
        assert_eq!(v, zero_vec);
        let Err(BurnError::InvalidDirection(v)) = Burn::new(*J2000, 60.0, 100.0, infinite_vec)
        else {
            panic!("Expected an invalid direction");
        };
        assert_eq!(v, infinite_vec);
        let Err(BurnError::InvalidDirection(v)) = Burn::new(*J2000, 60.0, 100.0, nan_vec) else {
            panic!("Expected an invalid direction");
        };
        assert!(v.x.is_nan());
    }
    // Correct error shape for negative, nan, and infinity
    #[test]
    fn invalid_duration() {
        let dir = DVec3::new(0.33, 0.77, -0.1);
        let negative_duration = -1.0;
        let nan_duration = f64::NAN;
        let infinite_duration = f64::INFINITY;
        let Err(BurnError::InvalidDuration(d)) = Burn::new(*J2000, negative_duration, 100.0, dir)
        else {
            panic!("Expected an invalid duration");
        };
        assert_eq!(d, negative_duration);
        let Err(BurnError::InvalidDuration(d)) = Burn::new(*J2000, nan_duration, 100.0, dir) else {
            panic!("Expected an invalid duration");
        };
        assert!(d.is_nan());
        let Err(BurnError::InvalidDuration(d)) = Burn::new(*J2000, infinite_duration, 100.0, dir)
        else {
            panic!("Expected an invalid duration");
        };
        assert_eq!(d, infinite_duration);
    }
    // Correct error shape for negative, nan, and infinity
    #[test]
    fn invalid_accel() {
        let dir = DVec3::new(0.33, 0.77, -0.1);
        let negative_accel = -5.0;
        let nan_accel = f64::NAN;
        let infinite_accel = f64::INFINITY;
        let Err(BurnError::InvalidAccel(a)) = Burn::new(*J2000, 60.0, negative_accel, dir) else {
            panic!("Expected an invalid accel");
        };
        assert_eq!(a, negative_accel);
        let Err(BurnError::InvalidAccel(a)) = Burn::new(*J2000, 60.0, nan_accel, dir) else {
            panic!("Expected an invalid accel");
        };
        assert!(a.is_nan());
        let Err(BurnError::InvalidAccel(a)) = Burn::new(*J2000, 60.0, infinite_accel, dir) else {
            panic!("Expected an invalid accel");
        };
        assert_eq!(a, infinite_accel);
    }
    // Zero accel and zero duration should remain usable
    #[test]
    fn zeros_are_ok() {
        let zero_accel_burn = Burn::new(*J2000, 60.0, 0.0, DVec3::new(0.1, -0.33, 0.97)).unwrap();
        assert_eq!(zero_accel_burn.duration(), 60.0);
        assert_eq!(zero_accel_burn.accel(), 0.0);
        let _zero_duration_burn =
            Burn::new(*J2000, 0.0, 100.0, DVec3::new(0.1, -0.33, 0.97)).unwrap();
    }
    // Bad JSON gets caught by try_from
    #[test]
    fn serde_rejection() {
        let json = r#"{"start":"2000-01-01T12:00:00 TT","duration":60.0,"accel":-5.0,"direction":[2.0,-3.0,6.0]}"#;
        let err = serde_json::from_str::<Burn>(json).unwrap_err();
        assert!(
            err.to_string().contains("must be finite"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fires_inside_window() {
        let burn = test_burn();
        let expected = DVec3::new(2.0, -3.0, 6.0) / 7.0 * burn.accel();
        let got = burn.accel_at(*J2000, 1.0);
        assert!(
            got.distance(expected) < 1e-12,
            "thrust {got} differs from expected {expected} by {}",
            got.distance(expected)
        );
    }

    #[test]
    fn zero_before_and_after() {
        let burn = test_burn();
        let before = *J2000 - 600.seconds();
        let after = *J2000 + 3600.seconds();
        assert_eq!(burn.accel_at(before, 13.0), DVec3::ZERO);
        assert_eq!(burn.accel_at(after, 25.0), DVec3::ZERO);
    }

    #[test]
    fn boundary_instants() {
        let burn = test_burn();
        assert_ne!(burn.accel_at(*J2000, 0.0), DVec3::ZERO);
        assert_ne!(burn.accel_at(*J2000, 899.0), DVec3::ZERO);
        assert_eq!(burn.accel_at(*J2000, 900.0), DVec3::ZERO);
    }
    #[test]
    fn reference_shift_invariance() {
        let burn = test_burn();
        let instant = *J2000 + 363.seconds();
        let ref_a = *J2000 - 137.seconds();
        let ref_b = *J2000 + 500.seconds();
        let a = burn.accel_at(ref_a, seconds_since(ref_a, instant));
        let b = burn.accel_at(ref_b, seconds_since(ref_b, instant));
        assert_eq!(a, b);
    }

    #[test]
    fn zero_duration_never_fires() {
        let burn = Burn::new(*J2000, 0.0, 3.2675, DVec3::new(2.0, -3.0, 6.0)).unwrap();
        assert_eq!(burn.accel_at(*J2000, 0.0), DVec3::ZERO);
    }
    fn test_burn() -> Burn {
        Burn::new(*J2000, 900.0, 3.2675, DVec3::new(2.0, -3.0, 6.0)).unwrap()
    }
}
