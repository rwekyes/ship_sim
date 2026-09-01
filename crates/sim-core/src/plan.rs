//! Flight plan containers
//! Serves both the planner and the UI

use crate::burns::Burn;
use serde::{Deserialize, Serialize};
/// May add variants for game reasons, like Waypoint
#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub enum Maneuver {
    Burn(Burn),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightPlan {
    maneuvers: Vec<Maneuver>,
}

impl FlightPlan {
    pub fn maneuvers(&self) -> impl Iterator<Item = &Maneuver> {
        self.maneuvers.iter()
    }
}
