//! Flight plan containers
//! Serves both the planner and the UI
use crate::burns::Burn;
/// May add variants for game reasons, like Waypoint
enum Maneuver{
    Burn(Burn),
}

struct FlightPlan{
    maneuvers: Vec<Maneuver>,
}

impl FlightPlan {
    pub fn maneuvers(&self) -> impl Iterator<Item = &Maneuver> {
        self.maneuvers.iter()
    }
}