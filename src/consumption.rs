use serde_derive::{Deserialize, Serialize};
use std::path::Path;

use crate::pvgis::PVArray;

#[derive(Clone, Copy, Deserialize, PartialEq, Serialize, Default)]
pub enum ConsumptionType {
  Sg,
  Es,
  #[default]
  None,
}

impl ConsumptionType {
  pub fn label(self) -> &'static str {
    match self {
      Self::Sg => "Stromnetz Graz",
      Self::Es => "Energie Steiermark",
      Self::None => "None",
    }
  }
}

pub fn load(path: &Path, kind: ConsumptionType) -> Result<Vec<u16>, String> {
  match kind {
    ConsumptionType::Sg => read_sg(path),
    ConsumptionType::Es => read_es(path),
    ConsumptionType::None => Err("CSV-Type not selected".into()),
  }
}

fn read_sg(path: &Path) -> Result<Vec<u16>, String> {
  let mut rdr = csv::ReaderBuilder::new()
    .delimiter(b';')
    .has_headers(false)
    .flexible(true)
    .from_path(path)
    .map_err(|error| error.to_string())?;
  let mut consumption = Vec::new();
  let mut acc: u16 = 0;

  for (i, res) in rdr.records().enumerate() {
    if i < 2 {
      continue;
    }

    let record = match res {
      Ok(record) => record,
      Err(_) => continue,
    };
    let Some(value) = record.get(2) else {
      continue;
    };
    let temp = value.trim();
    if temp.is_empty() {
      continue;
    }

    let wh = if temp.contains(',') {
      format!("{}0", temp.replace(',', ""))
    } else {
      temp.to_string()
    };
    acc = acc
      .checked_add(
        wh.parse::<u16>()
          .map_err(|error| format!("Invalid SG consumption value '{wh}': {error}"))?,
      )
      .ok_or_else(|| "SG consumption value exceeds 16-bit range".to_string())?;

    if (i + 2) % 4 == 0 {
      consumption.push(acc);
      acc = 0;
    }
  }

  Ok(consumption)
}

fn read_es(path: &Path) -> Result<Vec<u16>, String> {
  let mut rdr = csv::Reader::from_path(path).map_err(|error| error.to_string())?;
  let mut consumption = Vec::new();
  let mut acc: u16 = 0;

  for (i, res) in rdr.records().enumerate() {
    let record = res.map_err(|error| error.to_string())?;
    let Some(wh) = record.get(6) else {
      continue;
    };
    if wh.is_empty() {
      break;
    }
    acc = acc
      .checked_add(
        wh.parse::<u16>()
          .map_err(|error| format!("Invalid ES consumption value '{wh}': {error}"))?,
      )
      .ok_or_else(|| "ES consumption value exceeds 16-bit range".to_string())?;
    if (i + 1) % 4 == 0 {
      consumption.push(acc);
      acc = 0;
    }
  }

  Ok(consumption)
}

pub struct SimulationInput<'a> {
  pub power_consumption: &'a [u16],
  pub secondary_power_consumption: &'a [u16],
  pub power_production: &'a [PVArray],
  pub battery_kwh: f64,
  pub battery_discharge_cap: f64,
  pub inverter_max_output: f64,
  pub historic_prices: Option<&'a [f64]>,
}

#[derive(Clone, Default)]
pub struct MonthlySimulation {
  pub battery_load_percent: [f64; 12],
  pub used_from_grid: [f64; 12],
  pub used_from_grid_secondary: [f64; 12],
  pub input_to_grid: [f64; 12],
  pub input_to_secondary: [f64; 12],
  pub battery_empty_count: [usize; 12],
}

#[derive(Clone, Default)]
pub struct SimulationYear {
  pub monthly: MonthlySimulation,
  pub used_from_grid_total: f64,
  pub used_from_grid_secondary_total: f64,
  pub input_to_secondary_total: f64,
  pub input_to_grid_total: f64,
  pub historic_feed_in_revenue: f64,
}

pub fn simulate_environment(input: SimulationInput) -> Vec<SimulationYear> {
  let year_len = 365 * 24;
  let years = input.power_production[0].pvgis.len() / year_len;
  let mut results = Vec::with_capacity(years);
  for year in 0..years {
    let mut battery_charge = input.battery_kwh;
    let mut used_from_grid = 0.0;
    let mut used_from_grid_secondary = 0.0;
    let mut total_input = 0.0;
    let mut input_to_secondary = 0.0;
    let mut lowest_charge = input.battery_kwh;
    let mut emptied_list = vec![];
    let mut battery_charge_sum = [0.0; 12];
    let mut battery_charge_samples = [0usize; 12];
    let mut monthly_used_grid = [0.0; 12];
    let mut monthly_used_grid_secondary = [0.0; 12];
    let mut monthly_input_grid = [0.0; 12];
    let mut monthly_input_secondary = [0.0; 12];
    let mut monthly_empty_count = [0usize; 12];
    let mut historic_feed_in_revenue = 0.0;

    for hour in (year * 24 * 365)..((year + 1) * 24 * 365) {
      let hour_of_year = hour - year * 24 * 365;
      let month = month_at_hour(hour_of_year);
      let previous_used_grid = used_from_grid;
      let previous_used_grid_secondary = used_from_grid_secondary;
      let previous_total_input = total_input;
      let previous_input_secondary = input_to_secondary;
      let mut produce = input
        .power_production
        .iter()
        .map(|f| {
          (f.pvgis[hour].power_1kwp
            * (f64::from(f.panels.module_amount) * f.panels.module_wp / 1_000.0))
            / 1000.0
        })
        .sum::<f64>();

      let mut consumption = if input.power_consumption.len() > hour % 24 {
        input.power_consumption[hour % 24] as f64 / 1000.0
      } else {
        0.0
      };

      let mut sec_consumption = if input.secondary_power_consumption.len() > hour % 24 {
        input.secondary_power_consumption[hour % 24] as f64 / 1000.0
      } else {
        0.0
      };

      let mut battery_discharge_cap = input.battery_discharge_cap;
      let mut inverter_max_output = input.inverter_max_output;
      let inverter_max_input = input.inverter_max_output;

      // if we draw more power than the inverter can output, the rest has to come from the grid
      if consumption > inverter_max_output {
        used_from_grid += consumption - inverter_max_output;
        consumption = inverter_max_output;
      }

      // if the pv generates more power than the inverter can input then delete it :(
      // yes, strictly speaking this could also go into the battery. todo i guess
      if produce > inverter_max_input {
        produce = inverter_max_input;
      }

      if produce > consumption {
        // currently we produce more power than consume it
        inverter_max_output -= consumption;
        produce -= consumption;
        consumption = 0.0;

        if sec_consumption > inverter_max_output {
          used_from_grid_secondary += sec_consumption - inverter_max_output;
          sec_consumption = inverter_max_output;
        }

        if produce < sec_consumption {
          // we produce less power than we draw at our 2nd loc
          sec_consumption -= produce;
          inverter_max_output -= produce;
          input_to_secondary += produce;
          produce = 0.0;

          // again, our battery has limits how much it can output
          if sec_consumption > input.battery_discharge_cap {
            used_from_grid_secondary += sec_consumption - input.battery_discharge_cap;
            sec_consumption = input.battery_discharge_cap;
          }

          if battery_charge > sec_consumption {
            // we still have enough left in the battery to power sec location
            input_to_secondary += sec_consumption;
            battery_charge -= sec_consumption;
            inverter_max_output -= sec_consumption;
            sec_consumption = 0.0;
          } else if battery_charge < sec_consumption {
            // we don have enough left, so draw the rest from the grid
            input_to_secondary += battery_charge;
            sec_consumption -= battery_charge;
            inverter_max_output -= battery_charge;
            battery_charge = 0.0;
            used_from_grid_secondary += sec_consumption;
            sec_consumption = 0.0;
          } else {
            // somehow exactly the same again
            input_to_secondary += sec_consumption;
            sec_consumption = 0.0;
            battery_charge = 0.0;
          }
        } else {
          // we produce more power than we currently draw at our 2nd loc
          input_to_secondary += sec_consumption;
          produce -= sec_consumption;
          inverter_max_output -= sec_consumption;
          sec_consumption = 0.0;
        }

        // we try to charge the battery with the remaining power.
        // the one spec of battery that I looked at charges and discharges at the same rate
        // so we should be fine using the same cap variable here
        if produce > input.battery_discharge_cap {
          total_input += produce - input.battery_discharge_cap;
          produce = input.battery_discharge_cap;
        }

        if battery_charge == input.battery_kwh {
          // battery is already full, input to grid
          total_input += produce;
          produce = 0.0;
        } else {
          if battery_charge + produce > input.battery_kwh {
            // when charging the battery some power is left for the grid
            total_input += produce - (input.battery_kwh - battery_charge);
            battery_charge = input.battery_kwh;
            produce = 0.0;
          } else {
            // we charge the battery with the remaining power, nothing is left
            battery_charge += produce;
            produce = 0.0;
          }
        }
      } else if produce < consumption {
        // currently we produce less power than we consume
        consumption -= produce;
        produce = 0.0;

        // if we draw more power than the battery could output, the rest has to come from the grid
        if consumption > input.battery_discharge_cap {
          used_from_grid += consumption - input.battery_discharge_cap;
          consumption = input.battery_discharge_cap;
        }

        if battery_charge > consumption {
          // we have enough battery power left to cover our consumption
          battery_discharge_cap -= consumption;
          battery_charge -= consumption;
          consumption = 0.0;

          // lets check if we have battery left to input for secondary loc
          // and again, our battery has limits how much it can output
          if sec_consumption > battery_discharge_cap {
            used_from_grid_secondary += sec_consumption - battery_discharge_cap;
            sec_consumption = battery_discharge_cap;
          }

          if battery_charge > sec_consumption {
            // we still have enough left in the battery to power sec location
            input_to_secondary += sec_consumption;
            battery_charge -= sec_consumption;
            sec_consumption = 0.0;
          } else if battery_charge < sec_consumption {
            // we don have enough battery left, so draw the rest from the grid
            input_to_secondary += battery_charge;
            sec_consumption -= battery_charge;
            battery_charge = 0.0;
            used_from_grid_secondary += sec_consumption;
            sec_consumption = 0.0;
          } else {
            // somehow exactly the same again
            input_to_secondary += sec_consumption;
            sec_consumption = 0.0;
            battery_charge = 0.0;
          }
        } else if battery_charge < consumption {
          // we dont have enough battery left, so we need to draw from grid
          consumption -= battery_charge;
          battery_charge = 0.0;

          used_from_grid += consumption;
          consumption = 0.0;

          used_from_grid_secondary += sec_consumption;
          sec_consumption = 0.0;
        } else {
          // somehow we have exactly enough battery left as we are consuming (at 1st loc)
          battery_charge = 0.0;
          consumption = 0.0;

          // we still need to power the second location
          used_from_grid_secondary += sec_consumption;
          sec_consumption = 0.0;
        }
      } else {
        // the amount of power produced and consumed somehow matches
        produce = 0.0;
        consumption = 0.0;

        // we still need to power the second location
        // used_from_grid_secondary += sec_consumption;
        // sec_consumption = 0.0;

        // again, our battery has limits how much it can output
        if sec_consumption > input.battery_discharge_cap {
          used_from_grid_secondary += sec_consumption - input.battery_discharge_cap;
          sec_consumption = input.battery_discharge_cap;
        }

        if battery_charge > sec_consumption {
          // we still have enough left in the battery to power sec location
          input_to_secondary += sec_consumption;
          battery_charge -= sec_consumption;
          inverter_max_output -= sec_consumption;
          sec_consumption = 0.0;
        } else if battery_charge < sec_consumption {
          // we don have enough left, so draw the rest from the grid
          input_to_secondary += battery_charge;
          sec_consumption -= battery_charge;
          inverter_max_output -= battery_charge;
          battery_charge = 0.0;
          used_from_grid_secondary += sec_consumption;
          sec_consumption = 0.0;
        } else {
          // somehow exactly the same again
          input_to_secondary += sec_consumption;
          sec_consumption = 0.0;
          battery_charge = 0.0;
        }
      }

      if consumption != 0.0 {
        eprintln!("This should not happen. 1");
      }
      if battery_charge < 0.0 || battery_charge > input.battery_kwh {
        eprintln!("This should not happen. 2");
      }
      if produce != 0.0 {
        eprintln!("This should not happen. 3");
      }
      if sec_consumption != 0.0 {
        eprintln!("This should not happen. 4");
      }
      if inverter_max_output < 0.0 {
        eprintln!("This should not happen. 5");
      }

      // check at every hour if this is our lowest battery state yet
      if battery_charge < lowest_charge {
        lowest_charge = battery_charge;
      }
      battery_charge_sum[month] += battery_charge;
      battery_charge_samples[month] += 1;

      // end of day has been reached
      if (hour + 1) % 24 == 0 && battery_charge == 0.0 {
        let day = (hour % (24 * 365)) / 24;
        emptied_list.push(day);
        monthly_empty_count[month] += 1;
      }
      monthly_used_grid[month] += used_from_grid - previous_used_grid;
      monthly_used_grid_secondary[month] += used_from_grid_secondary - previous_used_grid_secondary;
      monthly_input_grid[month] += total_input - previous_total_input;
      monthly_input_secondary[month] += input_to_secondary - previous_input_secondary;

      if let Some(price) = input
        .historic_prices
        .and_then(|prices| prices.get(hour))
        .copied()
      {
        let price_per_kwh = price / 1000.0;
        historic_feed_in_revenue += (total_input - previous_total_input) * price_per_kwh;
      }
    }
    // println!("{year}: L1 Used from grid {:.2} kWh", used_from_grid.abs());
    // println!(
    //   "{year}: L2 Used from grid {:.2} kWh",
    //   used_from_grid_secondary.abs()
    // );
    // println!("{year}: Input to 2nd loc {:.2} kWh", input_to_secondary);
    // println!("{year}: Input to grid {:.2} kWh", total_input);
    // println!(
    //   "{year}: Times battery emptied {}: {:?}",
    //   emptied_list.len(),
    //   emptied_list
    // );
    // println!("{year}: Lowest charge {lowest_charge:.2}");
    let mut battery_load_percent = [0.0; 12];
    for month in 0..12 {
      battery_load_percent[month] = if battery_charge_samples[month] == 0 {
        0.0
      } else if input.battery_kwh > 0.0 {
        battery_charge_sum[month] / battery_charge_samples[month] as f64 / input.battery_kwh * 100.0
      } else {
        0.0
      };
    }
    results.push(SimulationYear {
      monthly: MonthlySimulation {
        battery_load_percent,
        used_from_grid: monthly_used_grid,
        used_from_grid_secondary: monthly_used_grid_secondary,
        input_to_grid: monthly_input_grid,
        input_to_secondary: monthly_input_secondary,
        battery_empty_count: monthly_empty_count,
      },
      used_from_grid_total: used_from_grid.abs(),
      used_from_grid_secondary_total: used_from_grid_secondary.abs(),
      input_to_secondary_total: input_to_secondary,
      input_to_grid_total: total_input,
      historic_feed_in_revenue,
    });
  }
  results
}

fn month_at_hour(hour_of_year: usize) -> usize {
  let day = hour_of_year / 24;
  let month_lengths = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  let mut start_day = 0;
  for (month, &length) in month_lengths.iter().enumerate() {
    if day < start_day + length {
      return month;
    }
    start_day += length;
  }
  11
}
