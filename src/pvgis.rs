use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};

use crate::model::SolarPanel;

#[derive(Debug, Serialize, Deserialize)]
pub struct BroadbandReq {
  pub longitude: f64,
  pub latitude: f64,
  pub surface_orientation: i16,
  pub surface_tilt: u8,
  pub start_time: String,
  pub end_time: String,
  pub frequency: String,
  #[serde(rename = "peak-power")]
  pub peak_power: u16,
  pub verbose: u8,
  pub csv: String,
  pub system_efficiency: f64,
  pub photovoltaic_model: String,
  pub irradiance_source: String,
  pub horizon_profile: String,
}

#[derive(Clone)]
pub struct Hour {
  // pub ghi_inclined: f64, // Global Inclined Irradiance
  // pub direct_h: f64,     // Direct Horizontal Irradiance
  // pub diffuse_h: f64,    // Diffuse Horizontal Irradiance
  // pub temp: f64,         // Temperature
  pub power_1kwp: f64, // Power, per 1 kWp
}

pub struct PVArray {
  pub pvgis: Vec<Hour>,
  pub total: f64,
  pub panels: SolarPanel,
}

#[derive(Debug)]
pub enum PVGISError {
  JsonDecode,
  ParseError,
  RequestError,
}

pub fn get_data(
  panel: &SolarPanel,
  lon: f64,
  lat: f64,
  start_year: u16,
  end_year: u16,
) -> Result<PVArray, PVGISError> {
  let url =
    "https://photovoltaic-geographic-information-system.ec.europa.eu/api/v6/performance/broadband";

  let req_body = BroadbandReq {
    longitude: lon,
    latitude: lat,
    surface_orientation: panel.mounting_direction.azimut(),
    surface_tilt: panel.mounting_tilt,
    start_time: format!("{start_year}-01-01T00:00:00"),
    end_time: format!("{end_year}-12-31T23:59:00"),
    frequency: "Hourly".into(),
    peak_power: 1,
    verbose: 9,
    csv: "series.csv".into(),
    system_efficiency: 0.86,
    photovoltaic_model: "cSi:Integrated 2025".into(),
    irradiance_source: "SARAH-3".into(),
    horizon_profile: "PVGIS".into(),
  };

  let client = reqwest::blocking::Client::new();
  let request = client.get(url).query(&req_body).send();

  let response = if let Ok(it) = request {
    it
  } else {
    return Err(PVGISError::RequestError);
  };

  if response.status().as_u16() != 200 {
    eprintln!("{}", response.status().as_u16());
  }

  match response.text() {
    Ok(text) => {
      if let Ok((data, total)) = parse(&text) {
        Ok(PVArray {
          pvgis: data,
          panels: panel.clone(),
          total,
        })
      } else {
        Err(PVGISError::ParseError)
      }
    },
    _ => Err(PVGISError::JsonDecode),
  }
}

pub fn parse(text: &str) -> Result<(Vec<Hour>, f64), csv::Error> {
  let mut rdr = csv::Reader::from_reader(text.as_bytes());
  let headers = rdr.headers()?.clone();
  let idx: HashMap<&str, usize> = headers.iter().enumerate().map(|(i, h)| (h, i)).collect();
  let time_idx = idx.get("time").copied();
  let col = |rec: &csv::StringRecord, name: &str| -> f64 {
    rec
      .get(idx[name])
      .and_then(|s| s.parse().ok())
      .filter(|value: &f64| value.is_finite())
      .unwrap_or(0.0)
  };

  let mut out = Vec::new();
  for rec in rdr.records() {
    let r = rec?;
    if time_idx.and_then(|i| r.get(i)).is_some_and(is_leap_day) {
      continue;
    }
    out.push(Hour {
      // ghi_inclined: col(&r, "Global Inclined Irradiance"),
      // direct_h: col(&r, "Direct Horizontal Irradiance"),
      // diffuse_h: col(&r, "Diffuse Horizontal Irradiance"),
      // temp: col(&r, "Temperature"),
      power_1kwp: col(&r, "Power"),
    });
  }

  let total = out.iter().map(|f| f.power_1kwp / 1000.0).sum();
  Ok((out, total))
}

fn is_leap_day(time: &str) -> bool {
  let date = time.get(..8).unwrap_or_default();
  date.get(4..6) == Some("02") && date.get(6..8) == Some("29")
}
