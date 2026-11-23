use serde_derive::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{consumption::ConsumptionType, energy_charts::Bzn};

#[derive(Default, Deserialize, Serialize)]
pub struct Settings {
  pub project_directory: Option<PathBuf>,
  pub project_path: Option<PathBuf>,
  pub consumption_directory: Option<PathBuf>,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
pub enum MountingDirection {
  North,
  NorthEast,
  East,
  SouthEast,
  #[default]
  South,
  SouthWest,
  West,
  NorthWest,
}

impl MountingDirection {
  pub fn label(self) -> &'static str {
    match self {
      Self::North => "North",
      Self::NorthEast => "North-east",
      Self::East => "East",
      Self::SouthEast => "South-east",
      Self::South => "South",
      Self::SouthWest => "South-west",
      Self::West => "West",
      Self::NorthWest => "North-west",
    }
  }

  pub fn azimut(self) -> i16 {
    match self {
      Self::North => 0,
      Self::NorthEast => 45,
      Self::East => 90,
      Self::SouthEast => 135,
      Self::South => 180,
      Self::SouthWest => 225,
      Self::West => 270,
      Self::NorthWest => 315,
    }
  }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct SolarPanel {
  pub module_amount: u32,
  #[serde(alias = "module_kwp")]
  pub module_wp: f64,
  pub mounting_direction: MountingDirection,
  pub mounting_tilt: u8,
  pub fixed: bool,
}

#[derive(Default, Deserialize, Serialize)]
pub struct Project {
  pub battery_kwh: f64,
  #[serde(default)]
  pub panels: Vec<SolarPanel>,
  pub consumption_type: ConsumptionType,
  pub consumption_path: Option<PathBuf>,
  pub consumption_type_2: ConsumptionType,
  pub consumption_path_2: Option<PathBuf>,
  pub longitude: f64,
  pub latidute: f64,
  #[serde(default)]
  pub grid_cost_kwh: f64,
  #[serde(default)]
  pub grid_cost_kwh_secondary: f64,
  #[serde(default)]
  pub transfer_cost_kwh: f64,
  #[serde(default)]
  pub feed_in_revenue_kwh: f64,
  #[serde(default)]
  pub pv_cost: f64,
  #[serde(default)]
  pub pv_module_cost: f64,
  pub consumption_directory: Option<PathBuf>,
  pub start_year: u16,
  pub end_year: u16,
  pub battery_discharge_cap: f64,
  pub inverter_max_output: f64,
  pub use_historic_pricing: Option<bool>,
  pub historic_pricing_region: Option<Bzn>,
}
