use directories::ProjectDirs;
use std::path::PathBuf;

use crate::model::Settings;

pub fn read_settings() -> Result<Settings, String> {
  let path = settings_path()?;
  match std::fs::read_to_string(path) {
    Ok(contents) => serde_json::from_str(&contents).map_err(|error| error.to_string()),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
    Err(error) => Err(error.to_string()),
  }
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
  let path = settings_path()?;
  if let Some(directory) = path.parent() {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
  }
  let contents = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
  std::fs::write(path, contents).map_err(|error| error.to_string())
}

fn settings_path() -> Result<PathBuf, String> {
  ProjectDirs::from("", "", "solar-calculator")
    .map(|dirs| dirs.config_dir().join("settings.json"))
    .ok_or_else(|| "could not determine the platform config directory".to_string())
}
