use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PriceReq {
  pub bzn: String,
  pub start: String,
  pub end: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PriceRes {
  unix_seconds: Vec<i64>,
  price: Vec<f64>,
}

#[derive(Debug)]
pub enum EnergyChartsError {
  JsonDecode,
  ParseError,
  RequestError,
}

pub fn get_data(bzn: Bzn, start_year: u16, end_year: u16) -> Result<Vec<f64>, EnergyChartsError> {
  let url = "https://api.energy-charts.info/price";

  let req_body = PriceReq {
    bzn: bzn.json_val().into(),
    start: format!("{start_year}-01-01T00:00Z"),
    end: format!("{end_year}-12-31T23:59Z"),
  };

  let client = reqwest::blocking::Client::new();
  let request = client.get(url).query(&req_body).send();

  let response = if let Ok(it) = request {
    it
  } else {
    return Err(EnergyChartsError::RequestError);
  };

  if response.status().as_u16() != 200 {
    eprintln!("{}", response.status().as_u16());
  }

  match response.text() {
    Ok(text) => {
      if let Ok(data) = serde_json::from_str::<PriceRes>(&text) {
        if data.unix_seconds.len() != data.price.len() {
          return Err(EnergyChartsError::ParseError);
        }

        Ok(
          data
            .unix_seconds
            .into_iter()
            .zip(data.price)
            .filter_map(|(time, price)| (!is_leap_day(time)).then_some(price.max(0.0)))
            .collect(),
        )
      } else {
        Err(EnergyChartsError::ParseError)
      }
    },
    _ => Err(EnergyChartsError::JsonDecode),
  }
}

fn is_leap_day(unix_seconds: i64) -> bool {
  let days = (unix_seconds + 3600).div_euclid(86_400);
  (days - 789).rem_euclid(1_461) == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bzn {
  AT,
  BE,
  BG,
  CH,
  CZ,
  Delu,
  Deatlu,
  DK1,
  DK2,
  EE,
  ES,
  FI,
  FR,
  GR,
  HR,
  HU,
  IE,
  ITBrindisi,
  ITCalabria,
  ITCentreNorth,
  ITCentreSouth,
  ITFoggia,
  ITGr,
  ITNorth,
  ITNorthAT,
  ITNorthCH,
  ITNorthFR,
  ITNorthSI,
  ITPriolo,
  ITRossano,
  ITSacoac,
  ITSacodc,
  ITSardinia,
  ITSicily,
  ITSouth,
  LT,
  LV,
  ME,
  NL,
  NO1,
  NO2,
  NO2NSL,
  NO3,
  NO4,
  NO5,
  PL,
  PT,
  RO,
  RS,
  SE1,
  SE2,
  SE3,
  SE4,
  SI,
  SK,
  Uabei,
  Uaips,
}

impl Bzn {
  pub fn all() -> &'static [Bzn] {
    &[
      Self::AT,
      Self::BE,
      Self::BG,
      Self::CH,
      Self::CZ,
      Self::Delu,
      Self::Deatlu,
      Self::DK1,
      Self::DK2,
      Self::EE,
      Self::ES,
      Self::FI,
      Self::FR,
      Self::GR,
      Self::HR,
      Self::HU,
      Self::IE,
      Self::ITBrindisi,
      Self::ITCalabria,
      Self::ITCentreNorth,
      Self::ITCentreSouth,
      Self::ITFoggia,
      Self::ITGr,
      Self::ITNorth,
      Self::ITNorthAT,
      Self::ITNorthCH,
      Self::ITNorthFR,
      Self::ITNorthSI,
      Self::ITPriolo,
      Self::ITRossano,
      Self::ITSacoac,
      Self::ITSacodc,
      Self::ITSardinia,
      Self::ITSicily,
      Self::ITSouth,
      Self::LT,
      Self::LV,
      Self::ME,
      Self::NL,
      Self::NO1,
      Self::NO2,
      Self::NO2NSL,
      Self::NO3,
      Self::NO4,
      Self::NO5,
      Self::PL,
      Self::PT,
      Self::RO,
      Self::RS,
      Self::SE1,
      Self::SE2,
      Self::SE3,
      Self::SE4,
      Self::SI,
      Self::SK,
      Self::Uabei,
      Self::Uaips,
    ]
  }

  pub fn label(&self) -> &'static str {
    match self {
      Self::AT => "Austria",
      Self::BE => "Belgium",
      Self::BG => "Bulgaria",
      Self::CH => "Switzerland",
      Self::CZ => "Czech Republic",
      Self::Delu => "Germany, Luxembourg",
      Self::Deatlu => "Germany, Austria, Luxembourg",
      Self::DK1 => "Denmark 1",
      Self::DK2 => "Denmark 2",
      Self::EE => "Estionia",
      Self::ES => "Spain",
      Self::FI => "Finland",
      Self::FR => "France",
      Self::GR => "Greece",
      Self::HR => "Croatia",
      Self::HU => "Hungary",
      Self::IE => "Ireland (Single Electricity Market)",
      Self::ITBrindisi => "Italy Brindisi",
      Self::ITCalabria => "Italy Calabria",
      Self::ITCentreNorth => "Italy Centre North",
      Self::ITCentreSouth => "Italy Centre South",
      Self::ITFoggia => "Italy Foggia",
      Self::ITGr => "Italy Greece (interconnector)",
      Self::ITNorth => "Italy North",
      Self::ITNorthAT => "Italy North Austria (interconnector)",
      Self::ITNorthCH => "Italy North Switzerland (interconnector)",
      Self::ITNorthFR => "Italy North France (interconnector)",
      Self::ITNorthSI => "Italy North Slovenia (interconnector)",
      Self::ITPriolo => "Italy Priolo",
      Self::ITRossano => "Italy Rossano",
      Self::ITSacoac => "Italy Sardinia Corsica AC",
      Self::ITSacodc => "Italy Sardinia Corsica DC",
      Self::ITSardinia => "Italy Sardinia",
      Self::ITSicily => "Italy Sicily",
      Self::ITSouth => "Italy South",
      Self::LT => "Lithuania",
      Self::LV => "Latvia",
      Self::ME => "Montenegro",
      Self::NL => "Netherlands",
      Self::NO1 => "Norway 1",
      Self::NO2 => "Norway 2",
      Self::NO2NSL => "Norway North Sea Link",
      Self::NO3 => "Norway 3",
      Self::NO4 => "Norway 4",
      Self::NO5 => "Norway 5",
      Self::PL => "Poland",
      Self::PT => "Portugal",
      Self::RO => "Romania",
      Self::RS => "Serbia",
      Self::SE1 => "Sweden 1",
      Self::SE2 => "Sweden 2",
      Self::SE3 => "Sweden 3",
      Self::SE4 => "Sweden 4",
      Self::SI => "Slovenia",
      Self::SK => "Slovakia",
      Self::Uabei => "Ukraine Burshtyn Island",
      Self::Uaips => "Ukraine IPS",
    }
  }

  pub fn json_val(&self) -> &'static str {
    match self {
      Self::AT => "AT",
      Self::BE => "BE",
      Self::BG => "BG",
      Self::CH => "CH",
      Self::CZ => "CZ",
      Self::Delu => "DELU",
      Self::Deatlu => "DEATLU",
      Self::DK1 => "DK1",
      Self::DK2 => "DK2",
      Self::EE => "EE",
      Self::ES => "ES",
      Self::FI => "FI",
      Self::FR => "FR",
      Self::GR => "GR",
      Self::HR => "HR",
      Self::HU => "HU",
      Self::IE => "IE(SEM)",
      Self::ITBrindisi => "ITBrindisi",
      Self::ITCalabria => "ITCalabria",
      Self::ITCentreNorth => "ITCentreNorth",
      Self::ITCentreSouth => "ITCentreSouth",
      Self::ITFoggia => "ITFoggia",
      Self::ITGr => "ITGR",
      Self::ITNorth => "ITNorth",
      Self::ITNorthAT => "ITNorthAT",
      Self::ITNorthCH => "ITNorthCH",
      Self::ITNorthFR => "ITNorthFR",
      Self::ITNorthSI => "ITNorthSI",
      Self::ITPriolo => "ITPriolo",
      Self::ITRossano => "ITRossano",
      Self::ITSacoac => "ITSACOAC",
      Self::ITSacodc => "ITSACODC",
      Self::ITSardinia => "ITSardinia",
      Self::ITSicily => "ITSicily",
      Self::ITSouth => "ITSouth",
      Self::LT => "LT",
      Self::LV => "LV",
      Self::ME => "ME",
      Self::NL => "NL",
      Self::NO1 => "NO1",
      Self::NO2 => "NO2",
      Self::NO2NSL => "NO2NSL",
      Self::NO3 => "NO3",
      Self::NO4 => "NO4",
      Self::NO5 => "NO5",
      Self::PL => "PL",
      Self::PT => "PT",
      Self::RO => "RO",
      Self::RS => "RS",
      Self::SE1 => "SE1",
      Self::SE2 => "SE2",
      Self::SE3 => "SE3",
      Self::SE4 => "SE4",
      Self::SI => "SI",
      Self::SK => "SK",
      Self::Uabei => "UABEI",
      Self::Uaips => "UAIPS",
    }
  }
}
