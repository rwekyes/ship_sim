//! Properties of planetary bodies
//! Source for μ - JPL DE440, ssd.jpl.nasa.gov/astro_par.html

use serde::{Deserialize, Serialize};
/// Central Body wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CentralBody {
    Sol,
    Mercury,
    Venus,
    Earth,
    Luna,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
    Pluto,
}

impl CentralBody {
    /// Returns central bodies mu for the purposes of a negligible mass object
    /// For a planet use the raw const below
    /// From Mars outward mu includes the moons
    pub fn mu(self) -> f64 {
        match self {
            CentralBody::Sol => MU_SOL,
            CentralBody::Mercury => MU_MERCURY,
            CentralBody::Venus => MU_VENUS,
            CentralBody::Earth => MU_EARTH,
            CentralBody::Luna => MU_LUNA,
            CentralBody::Mars => MU_MARS,
            CentralBody::Jupiter => MU_JUPITER,
            CentralBody::Saturn => MU_SATURN,
            CentralBody::Uranus => MU_URANUS,
            CentralBody::Neptune => MU_NEPTUNE,
            CentralBody::Pluto => MU_PLUTO,
        }
    }
}
/// Sun: μ = 1.32712440041279419 x 10²⁰ m³/s² - Rounded for f64 precision
pub(crate) const MU_SOL: f64 = 1.327_124_400_412_794_2e20;
/// Mercury: μ = 22031.868551 x 10⁹ km³/s²
pub(crate) const MU_MERCURY: f64 = 2.2031868551e13;
/// Venus: μ = 324858.592000 x 10⁹ km³/s²
pub(crate) const MU_VENUS: f64 = 3.24858592000e14;
/// Earth: μ = 398600.435507 x 10⁹ km³/s²
pub(crate) const MU_EARTH: f64 = 3.986_004_355_07e14;
/// Luna: μ = 4902.800118 x 10⁹ km³/s²
pub(crate) const MU_LUNA: f64 = 4.902800118e12;
/// Mars system: μ = 42828.375816 x 10⁹ km³/s²
pub(crate) const MU_MARS: f64 = 4.282_837_581_6e13;
/// Jupiter system: μ = 126712764.1 x 10⁹ km³/s²
pub(crate) const MU_JUPITER: f64 = 1.267127641e17;
/// Saturn system: μ = 37940584.841800 x 10⁹ km³/s²
pub(crate) const MU_SATURN: f64 = 3.7940584841800e16;
/// Uranus system: μ = 5794556.400000 x 10⁹ km³/s²
pub(crate) const MU_URANUS: f64 = 5.794556400000e15;
/// Neptune system: μ = 6836527.100580 x 10⁹ km³/s²
pub(crate) const MU_NEPTUNE: f64 = 6.836527100580e15;
/// Pluto system: μ = 975.500000 x 10⁹ km³/s²
pub(crate) const MU_PLUTO: f64 = 9.75500000e11;
