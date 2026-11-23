use crate::{
  consumption::{self, ConsumptionType, SimulationInput, SimulationYear, simulate_environment},
  energy_charts::{Bzn, EnergyChartsError, get_data as get_historic_prices},
  model::{MountingDirection, Project, Settings, SolarPanel},
  pvgis::{PVArray, get_data},
  storage,
};
use eframe::egui::{self, Color32, Ui};
use egui_plot::{Bar, BarChart, Legend, Plot, uniform_grid_spacer};
use std::{
  ops::RangeInclusive,
  path::{Path, PathBuf},
  sync::mpsc::{self, Receiver},
};

enum PVGISUpdate {
  Progress(usize),
  Finished(Vec<Result<PVArray, crate::pvgis::PVGISError>>),
}

enum HistoricPriceUpdate {
  Finished(Result<Vec<f64>, EnergyChartsError>),
}

struct App {
  battery_kwh: f64,
  battery_discharge_cap: f64,
  inverter_max_output: f64,
  grid_cost_kwh: f64,
  grid_cost_kwh_secondary: f64,
  transfer_cost_kwh: f64,
  feed_in_revenue_kwh: f64,
  use_historic_pricing: bool,
  historic_pricing_region: Bzn,
  historic_prices: Option<Vec<f64>>,
  historic_prices_loading: bool,
  historic_prices_error: Option<String>,
  historic_prices_receiver: Option<Receiver<HistoricPriceUpdate>>,
  pv_cost: f64,
  pv_module_cost: f64,
  panels: Vec<SolarPanel>,
  project_path: Option<PathBuf>,
  consumption_type: ConsumptionType,
  consumption_path: Option<PathBuf>,
  consumption_type_2: ConsumptionType,
  consumption_path_2: Option<PathBuf>,
  consumption_2: Vec<u16>,
  consumption_directory: Option<PathBuf>,
  consumption: Vec<u16>,
  project_directory: Option<PathBuf>,
  message: String,
  message_2: String,
  longitude: f64,
  latidute: f64,
  power_production: Vec<PVArray>,
  pvgis_error: Option<String>,
  start_year: u16,
  start_year_copy: u16,
  end_year: u16,
  needs_simulation: bool,
  needs_update: bool,
  pvgis_receiver: Option<Receiver<PVGISUpdate>>,
  pvgis_loading: bool,
  pvgis_progress: usize,
  monthly_totals: Vec<[f64; 12]>,
  hourly_averages: Vec<[f64; 24]>,
  yearly_sums: Vec<Vec<Vec<f64>>>,
  simulation_results: Vec<SimulationYear>,
}

impl Default for App {
  fn default() -> Self {
    Self::default_with_config(false)
  }
}

impl App {
  fn default_with_config(ignore_config: bool) -> Self {
    if ignore_config {
      return Self::default_without_settings();
    }

    match storage::read_settings() {
      Ok(settings) => {
        let mut app = Self {
          project_directory: settings.project_directory,
          consumption_directory: settings.consumption_directory,
          project_path: settings.project_path,
          ..Self::default_without_settings()
        };

        if app.project_directory.is_some() {
          app.load_project(true);
        }

        app
      },
      Err(error) => Self {
        message: format!("Could not load settings: {error}"),
        ..Self::default_without_settings()
      },
    }
  }

  fn default_without_settings() -> Self {
    Self {
      battery_kwh: 12.0,
      inverter_max_output: 0.0,
      battery_discharge_cap: 0.0,
      panels: Vec::new(),
      project_path: None,
      consumption_type: ConsumptionType::Es,
      consumption_path: None,
      consumption: Vec::new(),
      consumption_type_2: ConsumptionType::default(),
      consumption_path_2: None,
      consumption_2: Vec::new(),
      project_directory: None,
      consumption_directory: None,
      message: String::new(),
      message_2: String::new(),
      latidute: 51.455375,
      longitude: -0.958855,
      power_production: Vec::new(),
      pvgis_error: None,
      start_year: 2020,
      end_year: 2022,
      needs_simulation: false,
      grid_cost_kwh: 30.0,
      needs_update: false,
      pvgis_receiver: None,
      pvgis_loading: false,
      pvgis_progress: 0,
      monthly_totals: vec![],
      hourly_averages: vec![],
      yearly_sums: vec![],
      simulation_results: vec![],
      grid_cost_kwh_secondary: 30.0,
      transfer_cost_kwh: 0.0,
      feed_in_revenue_kwh: 0.0,
      pv_cost: 0.0,
      pv_module_cost: 0.0,
      start_year_copy: 2020,
      use_historic_pricing: false,
      historic_pricing_region: Bzn::AT,
      historic_prices: None,
      historic_prices_loading: false,
      historic_prices_error: None,
      historic_prices_receiver: None,
    }
  }
}

impl eframe::App for App {
  fn ui(&mut self, ui: &mut Ui, _: &mut eframe::Frame) {
    self.poll_pvgis_updates(ui.ctx());
    self.poll_historic_price_updates(ui.ctx());
    egui::Panel::left("options").show(ui, |ui| {
      egui::Panel::bottom("options_actions").show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
          let button = egui::Button::new("Solve");
          let response = ui.add_enabled(
            !self.pvgis_loading && !self.historic_prices_loading,
            button,
          );

          if self.needs_update {
            ui.painter().rect_stroke(
              response.rect,
              ui.visuals().widgets.inactive.corner_radius,
              egui::Stroke::new(2.0, egui::Color32::RED),
              egui::StrokeKind::Inside,
            );
          }

          if response.clicked() {
            self.save_settings_with_message();
            self.start_power_production_load();
            self.start_historic_price_load();
            self.needs_update = false;
            self.needs_simulation = true;
          }
          ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Save").clicked() {
              if self.project_path.is_some() {
                self.save_project();
              } else {
                self.save_settings_with_message();
              }
            }
          });
        });
        if self.pvgis_loading {
          let panel_count = self.panels.len().max(1);
          ui.label("Loading PVGIS data...");
          ui.add(
            egui::ProgressBar::new(self.pvgis_progress as f32 / panel_count as f32).text(format!(
              "{} / {}",
              self.pvgis_progress,
              self.panels.len()
            )),
          );
        }
        if self.historic_prices_loading {
          ui.label("Loading historic dynamic prices...");
        }
        ui.add_space(3.0);
      });

      egui::scroll_area::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("Project");
        ui.horizontal(|ui| {
          if ui.button("Load").clicked() {
            self.load_project(false);
          }
          if ui.button("Save").clicked() {
            self.save_project();
          }
        });
        if let Some(path) = &self.project_path {
          ui.small(path.display().to_string());
        }
        if !self.message.is_empty() {
          ui.small(&self.message);
        }

        ui.separator();
        ui.heading("PV Location");
        ui.add_space(3.0);
        ui.vertical(|ui| {
          ui.label(egui::RichText::new("Consumption Data:").underline());
          ui.horizontal(|ui| {
            ui.label("CSV Format:");
            egui::ComboBox::from_label("")
              .selected_text(self.consumption_type.label())
              .show_ui(ui, |ui| {
                if ui
                  .selectable_value(
                    &mut self.consumption_type,
                    ConsumptionType::None,
                    ConsumptionType::None.label(),
                  )
                  .changed()
                {
                  self.consumption.clear();
                  self.needs_simulation = true;
                }
                let mut consumption_type_changed = ui
                  .selectable_value(
                    &mut self.consumption_type,
                    ConsumptionType::Es,
                    ConsumptionType::Es.label(),
                  )
                  .changed();
                consumption_type_changed |= ui
                  .selectable_value(
                    &mut self.consumption_type,
                    ConsumptionType::Sg,
                    ConsumptionType::Sg.label(),
                  )
                  .changed();
                if consumption_type_changed {
                  self.load_consumption(true);
                  self.needs_simulation = true;
                }
              });
          });
          ui.horizontal(|ui| {
            if ui.button("Load csv...").clicked() {
              self.load_consumption(false);
            }
            if self.consumption_path.is_some() {
              ui.small(format!("{} values loaded", self.consumption.len()));
            }
          });
          ui.add_space(3.0);
        });

        ui.label(egui::RichText::new("PV Coordinates:").underline());

        ui.horizontal(|ui2| {
          ui2.label("Latidute:");
          let change = ui2
            .add(
              egui::DragValue::new(&mut self.latidute)
                .custom_formatter(|n, _range| format!("{n}"))
                .custom_parser(|s| s.parse::<f64>().ok()),
            )
            .changed();
          self.needs_update |= change;
        });
        ui.horizontal(|ui2| {
          ui2.label("Longitude:");
          let change = ui2
            .add(
              egui::DragValue::new(&mut self.longitude)
                .custom_formatter(|n, _range| format!("{n}"))
                .custom_parser(|s| s.parse::<f64>().ok()),
            )
            .changed();
          self.needs_update |= change;
        });

        ui.add_space(6.0);

        ui.label(egui::RichText::new("Simulation Time Range:").underline()).on_hover_text_at_pointer("The (inclusive) time range being used when requesting the data from PVGIS.");
        // Specify which start and end year should be used to estimate the power production
        ui.horizontal(|ui2| {
          ui2.label("Start year:");
          let change = ui2
            .add(egui::DragValue::new(&mut self.start_year).range(RangeInclusive::new(2005, 2025)))
            .changed();
          self.needs_update |= change;
        });

        ui.horizontal(|ui2| {
          ui2.label("End year:");
          let change = ui2
            .add(egui::DragValue::new(&mut self.end_year).range(RangeInclusive::new(2005, 2025)))
            .changed();
          self.needs_update |= change;
        });

        ui.add_space(6.0);

        ui.label(egui::RichText::new("PV Configurator:").underline());
        // Show the photvoltaic panel widget window
        self.show_panels_widget(ui);

        ui.add_space(6.0);

        ui.label(egui::RichText::new("Battery Configuration:").underline());
        // Battery options
        ui.horizontal(|ui| {
          ui.label("Battery size:");
          self.needs_simulation |= ui
            .add(
              egui::DragValue::new(&mut self.battery_kwh)
                .suffix(" kWh")
                .fixed_decimals(2)
                .range(0.0..=1000.0)
                .speed(0.1),
            )
            .changed();
        });
        ui.horizontal(|ui| {
            ui.label("Price per PV module:");
            ui.add(egui::DragValue::new(&mut self.pv_module_cost).suffix(" €").fixed_decimals(2).range(0.0..=1_000_000.0));
        });

        ui.horizontal(|ui| {
          ui.label("Battery (dis)charge capacity:");
          self.needs_simulation |= ui
            .add(
              egui::DragValue::new(&mut self.battery_discharge_cap)
                .suffix(" kWh")
                .fixed_decimals(2)
                .range(0.0..=1000.0)
                .speed(0.1),
            )
            .changed();
        });

        ui.add_space(6.0);

        ui.label(egui::RichText::new("Inverter Configuration:").underline());

        ui.horizontal(|ui| {
          ui.label("Inverter max. output power:");
          self.needs_simulation |= ui
            .add(
              egui::DragValue::new(&mut self.inverter_max_output)
                .suffix(" kW")
                .fixed_decimals(2)
                .range(0.0..=1000.0)
                .speed(0.1),
            )
            .changed();
        });

        ui.add_space(6.0);

        ui.label(egui::RichText::new("Cost Configuration:").underline());
        ui.horizontal(|ui| {
          ui.label("Total PV system cost:");
          ui.add(
            egui::DragValue::new(&mut self.pv_cost)
              .suffix(" €")
              .fixed_decimals(2)
              .range(0.0..=1_000_000.0)
              .speed(100.0),
          )
          .changed();
        });

        ui.horizontal(|ui| {
          ui.label("Grid ct/kWh:").on_hover_text_at_pointer("How much each kWh costs at the PV location. (for when the PV is not generating enough power and the battery is empty)");
          ui.add(
            egui::DragValue::new(&mut self.grid_cost_kwh)
              .suffix(" ct")
              .fixed_decimals(1)
              .range(0.0..=100.0)
              .speed(1),
          )
          .changed();
        });
        ui.group(|ui| {
          let hover = "We get the historic dynamic prices from 'energy-charts.info' to better estimate the prices that would have been payed out in a specific hour. BE CAREFUL! Prices dropped A LOT in the past two years.";

          ui.horizontal(|ui| {
            ui.label("Dynamic prices:").on_hover_text_at_pointer(hover);
            if ui
              .checkbox(&mut self.use_historic_pricing, "")
              .on_hover_text_at_pointer(hover)
              .changed()
            {
              self.needs_update = true;
              self.needs_simulation = true;
            }
          });

          ui.add_enabled_ui(!self.use_historic_pricing, |ui| {
            ui.horizontal(|ui| {
              ui.label("Feed-in ct/kWh:").on_hover_text_at_pointer("How much you get for each kWh that is fed into the system (and not used by your 2nd location).");
              ui.add(
                egui::DragValue::new(&mut self.feed_in_revenue_kwh)
                  .suffix(" ct")
                  .fixed_decimals(1)
                  .range(0.0..=100.0)
                  .speed(1),
              )
              .changed();
            });
          });
          ui.add_enabled_ui(self.use_historic_pricing, |ui| {
            ui.horizontal(|ui| {
              ui.label("Dynamic prices region:");
              egui::ComboBox::from_id_salt("dyn_hist_prices")
              .selected_text(self.historic_pricing_region.label())
              .show_ui(ui, |ui| {
                for region in Bzn::all() {
                  if ui
                  .selectable_value(
                    &mut self.historic_pricing_region,
                    *region,
                    region.label(),
                  )
                  .changed()
                  {
                    self.historic_prices = None;
                    self.needs_update = true;
                    self.needs_simulation = true;
                  }
                }
              });
            });
            if let Some(error) = &self.historic_prices_error {
              ui.colored_label(Color32::RED, error);
            }
          });
        });

        ui.separator();
        ui.heading("2nd Location");
        ui.vertical(|ui| {
          ui.label(egui::RichText::new("Consumption Data:").underline());
          ui.horizontal(|ui| {
            ui.label("CSV Format:");
            egui::ComboBox::from_id_salt("secondary_consumption")
              .selected_text(self.consumption_type_2.label())
              .show_ui(ui, |ui| {
                if ui
                  .selectable_value(
                    &mut self.consumption_type_2,
                    ConsumptionType::None,
                    ConsumptionType::None.label(),
                  )
                  .changed()
                {
                  self.consumption_2.clear();
                  self.needs_simulation = true;
                }
                let mut consumption_type_changed = ui
                  .selectable_value(
                    &mut self.consumption_type_2,
                    ConsumptionType::Es,
                    ConsumptionType::Es.label(),
                  )
                  .changed();
                consumption_type_changed |= ui
                  .selectable_value(
                    &mut self.consumption_type_2,
                    ConsumptionType::Sg,
                    ConsumptionType::Sg.label(),
                  )
                  .changed();
                if consumption_type_changed {
                  self.load_consumption_2(true);
                  self.needs_simulation = true;
                }
              });
          });
          ui.horizontal(|ui| {
            if ui.button("Load csv...").clicked() {
              self.load_consumption_2(false);
            }
            if self.consumption_path_2.is_some() {
              ui.small(format!("{} values loaded", self.consumption_2.len()));
            }
          });
          ui.add_space(3.0);
        });

        ui.label(egui::RichText::new("Cost Configuration:").underline());

        ui.horizontal(|ui| {
          ui.label("Grid ct/kWh:").on_hover_text_at_pointer("How much each kWh costs at the 2nd location, when the PV/Battery is currently not feeding power into the grid.");
          ui.add(
            egui::DragValue::new(&mut self.grid_cost_kwh_secondary)
              .suffix(" ct")
              .fixed_decimals(1)
              .range(0.0..=100.0)
              .speed(1),
          )
          .changed();
        });
        ui.horizontal(|ui| {
          ui.label("Electricity Cost ct/kWh (PV to 2nd):").on_hover_text_at_pointer("How much each kWh costs at the 2nd location, when the PV/Battery is currently feeding at least the same amount of power into the grid.");
          ui.add(
            egui::DragValue::new(&mut self.transfer_cost_kwh)
              .suffix(" ct")
              .fixed_decimals(1)
              .range(-100.0..=100.0)
              .speed(1),
          )
          .changed();
        });

        ui.add_space(6.0);
      });
    });

    if self.needs_simulation && !self.power_production.is_empty() && !self.historic_prices_loading {
      let input = SimulationInput {
        power_consumption: &self.consumption,
        power_production: &self.power_production,
        battery_kwh: self.battery_kwh,
        battery_discharge_cap: self.battery_discharge_cap,
        inverter_max_output: self.inverter_max_output,
        secondary_power_consumption: &self.consumption_2,
        historic_prices: self
          .use_historic_pricing
          .then_some(self.historic_prices.as_deref())
          .flatten(),
      };
      self.simulation_results = simulate_environment(input);
    }

    egui::CentralPanel::default().show(ui, |ui| {
      egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
          if !self.simulation_results.is_empty() {
            self.show_simulation_widget(ui);
          }
          ui.columns(2, |columns| {
            if !self.consumption.is_empty() {
              self.show_consumption_widget(&mut columns[0]);
            }
            if !self.power_production.is_empty() {
              self.show_power_production_widget(&mut columns[1]);
            }
          });
        });
    });

    self.needs_simulation = false;
  }
}

impl App {
  fn project(&self) -> Project {
    Project {
      battery_kwh: self.battery_kwh,
      panels: self.panels.clone(),
      consumption_type: self.consumption_type,
      consumption_path: self.consumption_path.clone(),
      longitude: self.longitude,
      latidute: self.latidute,
      consumption_type_2: self.consumption_type_2,
      consumption_path_2: self.consumption_path_2.clone(),
      grid_cost_kwh: self.grid_cost_kwh,
      grid_cost_kwh_secondary: self.grid_cost_kwh_secondary,
      transfer_cost_kwh: self.transfer_cost_kwh,
      feed_in_revenue_kwh: self.feed_in_revenue_kwh,
      pv_cost: self.pv_cost,
      pv_module_cost: self.pv_module_cost,
      battery_discharge_cap: self.battery_discharge_cap,
      consumption_directory: self.consumption_directory.clone(),
      start_year: self.start_year,
      end_year: self.end_year,
      inverter_max_output: self.inverter_max_output,
      historic_pricing_region: Some(self.historic_pricing_region),
      use_historic_pricing: Some(self.use_historic_pricing),
    }
  }

  fn start_power_production_load(&mut self) {
    self.power_production.clear();
    self.pvgis_error = None;
    self.pvgis_progress = 0;
    self.pvgis_loading = true;

    let panels = self.panels.clone();
    let longitude = self.longitude;
    let latidute = self.latidute;
    let start_year = self.start_year;
    // we copy the start years, so that the numbers in the ui still make sense when you change the options
    // but dont actually click on solve yet
    self.start_year_copy = self.start_year;
    let end_year = self.end_year;
    let (sender, receiver) = mpsc::channel();
    self.pvgis_receiver = Some(receiver);

    std::thread::spawn(move || {
      let mut results = Vec::with_capacity(panels.len());
      for (index, panel) in panels.iter().enumerate() {
        results.push(get_data(panel, longitude, latidute, start_year, end_year));
        let _ = sender.send(PVGISUpdate::Progress(index + 1));
      }
      let _ = sender.send(PVGISUpdate::Finished(results));
    });
  }

  fn start_historic_price_load(&mut self) {
    if !self.use_historic_pricing {
      self.historic_prices_loading = false;
      return;
    }

    self.historic_prices = None;
    self.historic_prices_error = None;
    self.historic_prices_receiver = None;

    self.historic_prices_loading = true;
    let region = self.historic_pricing_region;
    let start_year = self.start_year;
    let end_year = self.end_year;
    let (sender, receiver) = mpsc::channel();
    self.historic_prices_receiver = Some(receiver);

    std::thread::spawn(move || {
      let result = get_historic_prices(region, start_year, end_year);
      let _ = sender.send(HistoricPriceUpdate::Finished(result));
    });
  }

  fn poll_historic_price_updates(&mut self, context: &egui::Context) {
    let mut finished = None;
    if let Some(receiver) = &self.historic_prices_receiver {
      while let Ok(update) = receiver.try_recv() {
        match update {
          HistoricPriceUpdate::Finished(result) => finished = Some(result),
        }
      }
    }

    if let Some(result) = finished {
      match result {
        Ok(prices) => {
          self.historic_prices = Some(prices);
          self.historic_prices_error = None;
        },
        Err(error) => {
          self.historic_prices = None;
          self.historic_prices_error = Some(format!("{error:?}"));
          self.message = format!("Could not load historic dynamic prices: {error:?}");
        },
      }
      self.historic_prices_loading = false;
      self.historic_prices_receiver = None;
      self.needs_simulation = true;
    } else if self.historic_prices_loading {
      context.request_repaint();
    }
  }

  fn poll_pvgis_updates(&mut self, context: &egui::Context) {
    let mut finished = None;
    if let Some(receiver) = &self.pvgis_receiver {
      while let Ok(update) = receiver.try_recv() {
        match update {
          PVGISUpdate::Progress(progress) => self.pvgis_progress = progress,
          PVGISUpdate::Finished(results) => finished = Some(results),
        }
      }
    }

    if let Some(results) = finished {
      self.power_production.clear();
      self.pvgis_error = None;
      for result in results {
        match result {
          Ok(array) => self.power_production.push(array),
          Err(err) => self.pvgis_error = Some(format!("{err:?}")),
        }
      }
      self.pvgis_loading = false;
      self.pvgis_receiver = None;
      self.needs_simulation = true;
    } else if self.pvgis_loading {
      context.request_repaint();
    }
  }

  fn show_power_production_widget(&mut self, ui: &mut Ui) {
    ui.group(|ui| {
      ui.heading("Power production");
      if let Some(error) = &self.pvgis_error {
        ui.small(error);
      }

      if self.needs_simulation {
        (self.monthly_totals, self.hourly_averages, self.yearly_sums) =
          self.power_production_plot_data();
      }

      let years = self.yearly_sums[0].len();
      let month_labels = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
      ];
      ui.label("Average Monthly Production".to_string());
      ui.add_space(5.0);
      Plot::new("monthly-production")
        .allow_scroll(false)
        .height(240.0)
        .legend(Legend::default())
        .x_axis_label("Month")
        .x_axis_formatter(move |mark, _| {
          let month = mark.value.round() as usize;
          month_labels
            .get(month.saturating_sub(1))
            .map(|label| (*label).to_owned())
            .unwrap_or_default()
        })
        .show(ui, |plot_ui| {
          let mut charts = Vec::new();
          for (array, totals) in self.monthly_totals.iter().enumerate() {
            let color = pv_array_color(array);
            let chart = BarChart::new(
              format!("PV{array}"),
              totals
                .iter()
                .enumerate()
                .map(|(month, &total)| Bar::new((month + 1) as f64, total))
                .collect(),
            )
            .color(color);
            let chart = if charts.is_empty() {
              chart
            } else {
              let previous = charts.iter().collect::<Vec<_>>();
              chart.stack_on(&previous)
            };
            charts.push(chart);
          }
          for chart in charts {
            plot_ui.bar_chart(chart);
          }
        });
      for (i, arr) in self.power_production.iter().enumerate() {
        ui.label(format!(
          "Average Yearly Production (PV{i}): {:.2} kWh",
          (arr.total * ((arr.panels.module_wp / 1000.0) * arr.panels.module_amount as f64))
            / years as f64
        ));
      }
      ui.add_space(5.0);
      if !self.power_production.is_empty() {
        ui.label(format!(
          "Total Average Yearly Production: {:.2} kWh",
          self
            .power_production
            .iter()
            .map(|f| f.total * ((f.panels.module_wp / 1000.0) * f.panels.module_amount as f64))
            .sum::<f64>()
            / years as f64
        ));
      }
      ui.add_space(8.0);
      ui.separator();

      ui.label("Average Daily Production".to_string());
      ui.add_space(5.0);
      Plot::new("hourly-production")
        .allow_scroll(false)
        .height(240.0)
        .legend(Legend::default())
        .x_axis_label("Hour")
        .x_grid_spacer(uniform_grid_spacer(|_| [1.0, 1.0, 1.0]))
        .x_axis_formatter(|mark, _| format!("{}", mark.value.round() as usize))
        .show(ui, |plot_ui| {
          let mut charts = Vec::new();
          for (array, averages) in self.hourly_averages.iter().enumerate() {
            let color = pv_array_color(array);
            let chart = BarChart::new(
              format!("PV{array}"),
              averages
                .iter()
                .enumerate()
                .map(|(hour, &average)| Bar::new(hour as f64, average))
                .collect(),
            )
            .color(color);
            let chart = if charts.is_empty() {
              chart
            } else {
              let previous = charts.iter().collect::<Vec<_>>();
              chart.stack_on(&previous)
            };
            charts.push(chart);
          }
          for chart in charts {
            plot_ui.bar_chart(chart);
          }
        });
      if !self.power_production.is_empty() {
        ui.label(format!(
          "Average Daily Production: {:.2} kWh",
          self
            .hourly_averages
            .iter()
            .map(|averages| averages.iter().sum::<f64>())
            .sum::<f64>()
        ));
      }
      ui.add_space(8.0);
      ui.separator();
      ui.add_space(8.0);

      if years == 1 {
        return;
      }
      let mut worst_year = 0;
      let mut best_year = 0;
      let mut smallest = f64::INFINITY;
      let mut biggest = f64::NEG_INFINITY;
      for year in 0..years {
        let mut produce = 0.0;
        for array in &self.yearly_sums {
          produce += array[year].iter().sum::<f64>();
        }

        if produce < smallest {
          smallest = produce;
          worst_year = year;
        }
        if produce > biggest {
          biggest = produce;
          best_year = year;
        }
      }

      for (plot_id, (year_index, kwh, title)) in [
        (worst_year, smallest, "Worst Performing Year"),
        (best_year, biggest, "Best Performing Year"),
      ]
      .into_iter()
      .enumerate()
      {
        let year = self.start_year_copy + year_index as u16;
        ui.label(format!("{title} ({year})"));
        ui.add_space(5.0);
        Plot::new(format!("yearly-production-{plot_id}"))
          .allow_scroll(false)
          .height(240.0)
          .legend(Legend::default())
          .x_axis_label("Month")
          .x_axis_formatter({
            move |mark, _| {
              let month = mark.value.round() as usize;
              month_labels
                .get(month.saturating_sub(1))
                .map(|label| (*label).to_owned())
                .unwrap_or_default()
            }
          })
          .show(ui, |plot_ui| {
            let mut charts = Vec::new();
            for (array, yearly_sums) in self.yearly_sums.iter().enumerate() {
              let color = pv_array_color(array);
              let chart = BarChart::new(
                format!("PV{array}"),
                yearly_sums[year_index]
                  .iter()
                  .enumerate()
                  .map(|(month, &total)| Bar::new((month + 1) as f64, total))
                  .collect(),
              )
              .color(color);
              let chart = if charts.is_empty() {
                chart
              } else {
                let previous = charts.iter().collect::<Vec<_>>();
                chart.stack_on(&previous)
              };
              charts.push(chart);
            }
            for chart in charts {
              plot_ui.bar_chart(chart);
            }
          });
        ui.label(format!("Total Production: {kwh:.2} kWh"));
        ui.add_space(8.0);
        if plot_id == 0 {
          ui.separator();
        }
      }
    });
  }

  fn power_production_plot_data(&self) -> (Vec<[f64; 12]>, Vec<[f64; 24]>, Vec<Vec<Vec<f64>>>) {
    let mut monthly_totals = Vec::with_capacity(self.power_production.len());
    let mut hourly_averages = Vec::with_capacity(self.power_production.len());

    // see `yearly_sums`
    let mut yearly_sums_arrays = vec![];

    for array in &self.power_production {
      let mut array_monthly_totals = [0.0; 12];
      let mut hourly_totals = [0.0; 24];
      let mut hourly_counts = [0_u32; 24];
      let installed_kwp = f64::from(array.panels.module_amount) * array.panels.module_wp / 1_000.0;
      let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
      let mut month = 0;
      let mut next_month_day = month_days[0];

      // for the monthly data we want to average it through the years
      let year_len = 365 * 24;
      let years = array.pvgis.len() / year_len;

      // for each year we save 12 counters of the production of each month
      // useful for displaying worst/best year plots and also average monthly produce
      let mut yearly_sums: Vec<Vec<f64>> = vec![vec![0.0_f64; 12]; years];

      let mut monthly_sum = 0.0;

      for (hour_index, hour) in array.pvgis.iter().enumerate() {
        if !hour.power_1kwp.is_finite() {
          continue;
        }
        let production = hour.power_1kwp * installed_kwp / 1_000.0;

        if hour_index < year_len {
          let day = hour_index / 24;
          while month < 11 && day >= next_month_day {
            array_monthly_totals[month] = monthly_sum / years as f64;
            monthly_sum = 0.0;

            next_month_day += month_days[month];
            month += 1;
          }

          for year in 0..years {
            let produce =
              array.pvgis[hour_index + year * year_len].power_1kwp * installed_kwp / 1_000.0;
            yearly_sums[year][month] += produce;
            monthly_sum += produce;
          }
        }

        let hour_of_day = hour_index % 24;
        hourly_totals[hour_of_day] += production;
        hourly_counts[hour_of_day] += 1;
      }

      array_monthly_totals[11] = monthly_sum / years as f64;
      monthly_totals.push(array_monthly_totals);

      hourly_averages.push(std::array::from_fn(|hour| {
        if hourly_counts[hour] == 0 {
          0.0
        } else {
          hourly_totals[hour] / f64::from(hourly_counts[hour])
        }
      }));

      yearly_sums_arrays.push(yearly_sums);
    }

    (monthly_totals, hourly_averages, yearly_sums_arrays)
  }

  fn show_panels_widget(&mut self, ui: &mut Ui) {
    ui.group(|ui| {
      if !self.panels.is_empty() {
        egui::ScrollArea::vertical()
          .id_salt("solar-panels")
          .max_height(440.0)
          .show(ui, |ui| {
            let mut remove_index = None;
            for (index, panel) in self.panels.iter_mut().enumerate() {
              ui.push_id(index, |ui| {
                ui.group(|ui| {
                  ui.label(egui::RichText::new(format!("PV{index}")).underline());
                  ui.add_enabled_ui(!panel.fixed, |ui| {
                    ui.horizontal(|ui| {
                      ui.label("Module amount:");
                      let inplace_changed = ui
                        .add(egui::DragValue::new(&mut panel.module_amount).range(0..=u32::MAX))
                        .changed();
                      if inplace_changed {
                        if let Some(array) = self.power_production.get_mut(index) {
                          array.panels.module_amount = panel.module_amount;
                        }
                        self.needs_simulation = true;
                      }
                    });
                  });
                  ui.horizontal(|ui| {
                    ui.label("Module Wp:");
                    let inplace_changed = ui
                      .add(
                        egui::DragValue::new(&mut panel.module_wp)
                          .range(0.0..=100_000.0)
                          .suffix(" Wp"),
                      )
                      .changed();
                    if inplace_changed {
                      if let Some(array) = self.power_production.get_mut(index) {
                        array.panels.module_wp = panel.module_wp;
                      }
                      self.needs_simulation = true;
                    }
                  });
                  ui.horizontal(|ui| {
                    ui.label("Mounting direction:");
                    egui::ComboBox::from_id_salt(("mounting-direction", index))
                      .selected_text(panel.mounting_direction.label())
                      .show_ui(ui, |ui| {
                        for direction in [
                          MountingDirection::North,
                          MountingDirection::NorthEast,
                          MountingDirection::East,
                          MountingDirection::SouthEast,
                          MountingDirection::South,
                          MountingDirection::SouthWest,
                          MountingDirection::West,
                          MountingDirection::NorthWest,
                        ] {
                          let change = ui
                            .selectable_value(
                              &mut panel.mounting_direction,
                              direction,
                              direction.label(),
                            )
                            .changed();
                          self.needs_update |= change;
                        }
                      });
                  });
                  ui.horizontal(|ui| {
                    ui.label("Mounting tilt:");
                    let change = ui
                      .add(
                        egui::DragValue::new(&mut panel.mounting_tilt)
                          .range(0.0..=90.0)
                          .suffix("°"),
                      )
                      .changed();
                    self.needs_update |= change;
                  });
                  ui.horizontal(|ui| {
                    if ui.small_button("Remove").clicked() {
                      remove_index = Some(index);
                      self.needs_simulation = true;
                    }
                  })
                });
              });
            }
            if let Some(index) = remove_index {
              self.panels.remove(index);
              if self.power_production.len() > index {
                self.power_production.remove(index);
              }
            }
          });
      } else {
        ui.label("No panels configured yet.");
      }

      ui.horizontal(|ui| {
        if ui.button("Add panel").clicked() {
          self.panels.push(SolarPanel {
            module_amount: 1,
            module_wp: 460.0,
            ..SolarPanel::default()
          });
          self.needs_update = true;
        }
      });
    });
  }

  fn show_simulation_widget(&mut self, ui: &mut Ui) {
    let year_count = self.simulation_results.len();
    let mut battery_load = [0.0; 12];
    let mut used_grid = [0.0; 12];
    let mut used_grid_secondary = [0.0; 12];
    let mut input_grid = [0.0; 12];
    let mut input_secondary = [0.0; 12];
    let mut empty_count = [0.0; 12];
    let mut used_grid_total = 0.0;
    let mut used_grid_secondary_total = 0.0;
    let mut transferred_total = 0.0;
    let mut sold_to_grid_total = 0.0;
    let mut historic_feed_in_revenue = 0.0;

    for result in &self.simulation_results {
      used_grid_total += result.used_from_grid_total;
      used_grid_secondary_total += result.used_from_grid_secondary_total;
      transferred_total += result.input_to_secondary_total;
      sold_to_grid_total += result.input_to_grid_total;
      historic_feed_in_revenue += result.historic_feed_in_revenue;
      for month in 0..12 {
        battery_load[month] += result.monthly.battery_load_percent[month];
        used_grid[month] += result.monthly.used_from_grid[month];
        used_grid_secondary[month] += result.monthly.used_from_grid_secondary[month];
        input_grid[month] += result.monthly.input_to_grid[month];
        input_secondary[month] += result.monthly.input_to_secondary[month];
        empty_count[month] += result.monthly.battery_empty_count[month] as f64;
      }
    }
    for month in 0..12 {
      battery_load[month] /= year_count as f64;
      used_grid[month] /= year_count as f64;
      used_grid_secondary[month] /= year_count as f64;
      input_grid[month] /= year_count as f64;
      input_secondary[month] /= year_count as f64;
      empty_count[month] /= year_count as f64;
    }
    used_grid_total /= year_count as f64;
    used_grid_secondary_total /= year_count as f64;
    transferred_total /= year_count as f64;
    sold_to_grid_total /= year_count as f64;
    historic_feed_in_revenue /= year_count as f64;

    let month_labels = [
      "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    ui.group(|ui| {
      ui.heading("Simulation results");
      ui.label(format!(
        "Monthly averages across {year_count} simulation year(s)"
      ));

      ui.separator();

      ui.columns(2, |columns| {
        columns[0].vertical(|ui| {
          ui.label("Average battery load");
          Plot::new("simulation-battery-load")
            .allow_scroll(false)
            .height(220.0)
            .legend(Legend::default())
            .x_axis_label("Month")
            .x_axis_formatter({
              let labels = month_labels;
              move |mark, _| {
                let month = mark.value.round() as usize;
                labels
                  .get(month.saturating_sub(1))
                  .map_or_else(String::new, |s| (*s).to_owned())
              }
            })
            .show(ui, |plot_ui| {
              plot_ui.bar_chart(
                BarChart::new(
                  "Battery load (%)",
                  battery_load
                    .iter()
                    .enumerate()
                    .map(|(month, &value)| Bar::new((month + 1) as f64, value))
                    .collect(),
                )
                .color(Color32::from_rgb(80, 180, 110)),
              );
            });
        });

        columns[1].vertical(|ui| {
          ui.label("Monthly kWh used from grid");
          Plot::new("simulation-grid-used")
            .allow_scroll(false)
            .height(220.0)
            .legend(Legend::default())
            .x_axis_label("Month")
            .x_axis_formatter({
              let labels = month_labels;
              move |mark, _| {
                let month = mark.value.round() as usize;
                labels
                  .get(month.saturating_sub(1))
                  .map_or_else(String::new, |s| (*s).to_owned())
              }
            })
            .show(ui, |plot_ui| {
              let first = BarChart::new(
                "1st location",
                used_grid
                  .iter()
                  .enumerate()
                  .map(|(month, &value)| Bar::new((month + 1) as f64, value))
                  .collect(),
              )
              .color(Color32::from_rgb(220, 130, 70));
              let second = BarChart::new(
                "2nd location",
                used_grid_secondary
                  .iter()
                  .enumerate()
                  .map(|(month, &value)| Bar::new((month + 1) as f64, value))
                  .collect(),
              )
              .color(Color32::from_rgb(100, 150, 220))
              .stack_on(&[&first]);
              plot_ui.bar_chart(first);
              plot_ui.bar_chart(second);
            });
        });
      });

      ui.columns(2, |columns| {
        columns[0].vertical(|ui| {
          ui.label("Monthly kWh input to grid");
          Plot::new("simulation-grid-input")
            .allow_scroll(false)
            .height(220.0)
            .legend(Legend::default())
            .x_axis_label("Month")
            .x_axis_formatter({
              let labels = month_labels;
              move |mark, _| {
                let month = mark.value.round() as usize;
                labels
                  .get(month.saturating_sub(1))
                  .map_or_else(String::new, |s| (*s).to_owned())
              }
            })
            .show(ui, |plot_ui| {
              let sold = BarChart::new(
                "Sold to grid",
                input_grid
                  .iter()
                  .enumerate()
                  .map(|(month, &value)| Bar::new((month + 1) as f64, value))
                  .collect(),
              )
              .color(Color32::from_rgb(180, 100, 220));
              let secondary = BarChart::new(
                "Sent to 2nd location",
                input_secondary
                  .iter()
                  .enumerate()
                  .map(|(month, &value)| Bar::new((month + 1) as f64, value))
                  .collect(),
              )
              .color(Color32::from_rgb(90, 190, 190))
              .stack_on(&[&sold]);
              plot_ui.bar_chart(sold);
              plot_ui.bar_chart(secondary);
            });
        });

        columns[1].vertical(|ui| {
          ui.label("Battery level reached 0%");
          Plot::new("simulation-battery-empty")
            .allow_scroll(false)
            .height(220.0)
            .legend(Legend::default())
            .x_axis_label("Month")
            .x_axis_formatter({
              let labels = month_labels;
              move |mark, _| {
                let month = mark.value.round() as usize;
                labels
                  .get(month.saturating_sub(1))
                  .map_or_else(String::new, |s| (*s).to_owned())
              }
            })
            .show(ui, |plot_ui| {
              plot_ui.bar_chart(
                BarChart::new(
                  "Times reached 0%",
                  empty_count
                    .iter()
                    .enumerate()
                    .map(|(month, &value)| Bar::new((month + 1) as f64, value))
                    .collect(),
                )
                .color(Color32::from_rgb(220, 80, 80)),
              );
            });
        });
      });
      ui.separator();
      ui.heading("Average yearly power costs");
      let first_location_cost = used_grid_total * self.grid_cost_kwh / 100.0;
      let second_grid_cost = used_grid_secondary_total * self.grid_cost_kwh_secondary / 100.0;
      let transfer_cost = transferred_total * self.transfer_cost_kwh / 100.0;
      let second_location_cost = second_grid_cost + transfer_cost;
      let both_locations_cost = first_location_cost + second_location_cost;
      let grid_revenue = if self.use_historic_pricing
        && self.historic_prices.is_some()
      {
        historic_feed_in_revenue
      } else {
        sold_to_grid_total * self.feed_in_revenue_kwh / 100.0
      };

      // apparently you can usually get money payed out for inputting to the grid so this number can be negativ.
      // not sure about the specifics yet though
      // let net_both_locations_cost = both_locations_cost - grid_revenue.min(both_locations_cost);
      let net_both_locations_cost = both_locations_cost - grid_revenue;

      ui.columns(2, |columns| {
        columns[0].vertical(|ui| {
          ui.label(format!(
            "1st location grid power: {:.2} € | {:.2} kWh",
            first_location_cost, used_grid_total
          ));
          ui.label(format!(
            "2nd location grid power: {:.2} € | {:.2} kWh",
            second_grid_cost, used_grid_secondary_total
          ));
          ui.label(format!(
            "Power sent to 2nd location: {:.2} € | {:.2} kWh",
            transfer_cost, transferred_total
          ));
          ui.label(format!(
            "Power sold to grid: {:.2} € | {:.2} kWh ({:.2} ct/kWh)",
            grid_revenue, sold_to_grid_total, grid_revenue / sold_to_grid_total * 100.0
          ));
        });
        columns[1].vertical(|ui| {
          ui.label(format!(
            "Total cost - 2nd location: {:.2} €",
            second_location_cost
          ));
          ui.label(format!(
            "Net total cost - both locations: {:.2} €",
            net_both_locations_cost
          )).on_hover_text_at_pointer("If this is negative, then yes, you should in theory be able to get this amount payed out.");
        });
      });

      ui.separator();
      ui.heading("PV payback");
      let first_consumption_kwh: f64 = (self
        .consumption
        .iter()
        .map(|&value| f64::from(value))
        .sum::<f64>()
        / 1_000.0)
        * 365.0;
      let second_consumption_kwh: f64 = (self
        .consumption_2
        .iter()
        .map(|&value| f64::from(value))
        .sum::<f64>()
        / 1_000.0)
        * 365.0;
      let cost_without_pv = first_consumption_kwh * self.grid_cost_kwh / 100.0
        + second_consumption_kwh * self.grid_cost_kwh_secondary / 100.0;
      let annual_savings = cost_without_pv - net_both_locations_cost;
      let module_cost = self.panels.iter().map(|panel| panel.module_amount).sum::<u32>() as f64 * self.pv_module_cost; let total_pv_cost = self.pv_cost + module_cost;

      ui.columns(2, |columns| {
        columns[0].vertical(|ui| {
          ui.label(format!(
            "Electricity Cost without PV - 1st location: {:.2} €",
            first_consumption_kwh * self.grid_cost_kwh / 100.0
          ));
          ui.label(format!(
            "Electricity Cost without PV - 2nd location: {:.2} €",
            second_consumption_kwh.abs() * self.grid_cost_kwh_secondary / 100.0
          ));
          ui.label(format!(
            "Electricity Cost without PV - both locations: {:.2} €",
            cost_without_pv
          ));
          ui.label(format!(
            "PV system cost: {:.2} € (base {:.2} € + modules {:.2} €)",
            total_pv_cost, self.pv_cost, module_cost
          ));
        });
        columns[1].vertical(|ui| {
          ui.label(format!("Average yearly savings: {:.2} €", annual_savings));
          if total_pv_cost <= 0.0 {
            ui.label("Payback time: no PV cost entered");
          } else if annual_savings > 0.0 {
            ui.label(format!(
              "Payback time: {:.2} years",
              total_pv_cost / annual_savings
            ));
          } else {
            ui.label("Payback time: never (no positive yearly savings)");
          }
        });
      });
    });
  }

  fn show_consumption_widget(&mut self, ui: &mut Ui) {
    ui.group(|ui| {
      ui.heading("Power consumption - PV Location");

      let bars = self
        .consumption
        .iter()
        .enumerate()
        .map(|(index, &value)| Bar::new(index as f64, f64::from(value)))
        .collect();

      let consumption_plot = Plot::new("power-consumption")
        .allow_scroll(false)
        .width(ui.available_width())
        .height(220.0)
        .legend(Legend::default())
        .show_crosshair(false)
        .x_axis_label("Hour")
        .x_grid_spacer(uniform_grid_spacer(|_| [1.0, 1.0, 1.0]))
        .x_axis_formatter(|mark, _| format!("{}", mark.value.round() as usize))
        .show(ui, |plot_ui| {
          plot_ui.bar_chart(BarChart::new("Consumption (Wh)", bars));
          plot_ui.pointer_coordinate().and_then(|pointer| {
            let hour = pointer.x.round();
            if hour.is_sign_negative() {
              return None;
            }
            let hour = hour as usize;
            self.consumption.get(hour).and_then(|&value| {
              if (pointer.x - hour as f64).abs() <= 0.5
                && pointer.y >= 0.0
                && pointer.y <= f64::from(value)
              {
                Some(hour)
              } else {
                None
              }
            })
          })
        });
      let hovered_hour = consumption_plot.inner;

      if let Some(hour) = hovered_hour {
        ui.label(format!("Hour {hour}: {} Wh", self.consumption[hour]));
      }

      let total_wh: u64 = self.consumption.iter().map(|&value| u64::from(value)).sum();
      ui.label(format!(
        "Total: {total_wh} Wh ({:.2} kWh)",
        total_wh as f64 / 1_000.0
      ));
      ui.label(format!(
        "Cost without PV: {:.2} kWh at {} ct/kWh = {:.2} €",
        total_wh as f64 / 1_000.0,
        self.grid_cost_kwh,
        total_wh as f64 / 1_000.0 * self.grid_cost_kwh / 100.0
      ));

      if !self.consumption_2.is_empty() {
        ui.separator();
        ui.heading("Power consumption - 2nd Location");

        let bars = self
          .consumption_2
          .iter()
          .enumerate()
          .map(|(index, &value)| Bar::new(index as f64, f64::from(value)))
          .collect();

        let consumption_plot_2 = Plot::new("power-consumption2")
          .allow_scroll(false)
          .width(ui.available_width())
          .height(220.0)
          .legend(Legend::default())
          .show_crosshair(false)
          .x_axis_label("Hour")
          .x_grid_spacer(uniform_grid_spacer(|_| [1.0, 1.0, 1.0]))
          .x_axis_formatter(|mark, _| format!("{}", mark.value.round() as usize))
          .show(ui, |plot_ui| {
            plot_ui.bar_chart(BarChart::new("Consumption (Wh)", bars));
            plot_ui.pointer_coordinate().and_then(|pointer| {
              let hour = pointer.x.round();
              if hour.is_sign_negative() {
                return None;
              }
              let hour = hour as usize;
              self.consumption_2.get(hour).and_then(|&value| {
                if (pointer.x - hour as f64).abs() <= 0.5
                  && pointer.y >= 0.0
                  && pointer.y <= f64::from(value)
                {
                  Some(hour)
                } else {
                  None
                }
              })
            })
          });
        let hovered_hour = consumption_plot_2.inner;

        if let Some(hour) = hovered_hour {
          ui.label(format!("Hour {hour}: {} Wh", self.consumption_2[hour]));
        }

        let total_wh: u64 = self
          .consumption_2
          .iter()
          .map(|&value| u64::from(value))
          .sum();
        ui.label(format!(
          "Total: {total_wh} Wh ({:.2} kWh)",
          total_wh as f64 / 1_000.0
        ));
        ui.label(format!(
          "Cost without PV: {:.2} kWh at {} ct/kWh = {:.2} €",
          total_wh as f64 / 1_000.0,
          self.grid_cost_kwh_secondary,
          total_wh as f64 / 1_000.0 * self.grid_cost_kwh_secondary / 100.0
        ));
      }
    });
  }

  fn load_consumption(&mut self, skip_dialog: bool) {
    let path;
    if !skip_dialog {
      let mut dialog = rfd::FileDialog::new().add_filter("CSV files", &["csv"]);
      if let Some(directory) = &self.consumption_directory {
        dialog = dialog.set_directory(directory);
      }
      if let Some(pathbuf) = dialog.pick_file() {
        path = pathbuf;
      } else {
        return;
      };
    } else {
      if let Some(ref pathbuf) = self.consumption_path {
        path = pathbuf.to_path_buf();
      } else {
        return;
      };
    }

    match consumption::load(&path, self.consumption_type) {
      Ok(values) => {
        self.consumption = values;
        self.consumption_path = Some(path);
        self.consumption_directory = Self::parent_directory(self.consumption_path.as_ref());
        self.needs_simulation = true;
      },
      Err(error) => self.message = format!("Could not load consumption CSV: {error}"),
    }
  }

  fn load_consumption_2(&mut self, skip_dialog: bool) {
    let path;
    if !skip_dialog {
      let mut dialog = rfd::FileDialog::new().add_filter("CSV files", &["csv"]);
      if let Some(directory) = &self.consumption_directory {
        dialog = dialog.set_directory(directory);
      }
      if let Some(pathbuf) = dialog.pick_file() {
        path = pathbuf;
      } else {
        return;
      };
    } else {
      if let Some(ref pathbuf) = self.consumption_path_2 {
        path = pathbuf.to_path_buf();
      } else {
        return;
      };
    }

    match consumption::load(&path, self.consumption_type_2) {
      Ok(values) => {
        self.consumption_2 = values;
        self.consumption_path_2 = Some(path);
        self.needs_simulation = true;
      },
      Err(error) => self.message_2 = format!("Could not load consumption CSV: {error}"),
    }
  }

  fn load_project(&mut self, skip_dialog: bool) {
    let path;
    if !skip_dialog {
      let mut dialog = rfd::FileDialog::new().add_filter("Solar project", &["json"]);
      if let Some(directory) = &self.project_directory {
        dialog = dialog.set_directory(directory);
      }
      if let Some(pathbuf) = dialog.pick_file() {
        path = pathbuf;
      } else {
        return;
      }
    } else {
      if let Some(ref pathbuf) = self.project_path {
        path = pathbuf.to_path_buf();
      } else {
        return;
      };
    }

    match std::fs::read_to_string(&path) {
      Ok(contents) => match serde_json::from_str::<Project>(&contents) {
        Ok(project) => {
          self.battery_kwh = project.battery_kwh;
          self.panels = project.panels;
          self.consumption_type = project.consumption_type;
          self.consumption_path = project.consumption_path;
          self.consumption_type_2 = project.consumption_type_2;
          self.consumption_path_2 = project.consumption_path_2;
          self.longitude = project.longitude;
          self.latidute = project.latidute;
          self.grid_cost_kwh = if project.grid_cost_kwh == 0.0 {
            self.grid_cost_kwh
          } else {
            project.grid_cost_kwh
          };
          self.grid_cost_kwh_secondary = if project.grid_cost_kwh_secondary == 0.0 {
            self.grid_cost_kwh_secondary
          } else {
            project.grid_cost_kwh_secondary
          };
          self.transfer_cost_kwh = project.transfer_cost_kwh;
          self.feed_in_revenue_kwh = project.feed_in_revenue_kwh;
          self.pv_cost = project.pv_cost;
          self.pv_module_cost = project.pv_module_cost;
          self.consumption_directory = self.project_directory.clone();
          self.start_year = project.start_year;
          self.end_year = project.end_year;
          self.battery_discharge_cap = project.battery_discharge_cap;
          self.inverter_max_output = project.inverter_max_output;
          self.use_historic_pricing = project.use_historic_pricing.unwrap_or(false);
          self.historic_pricing_region = project.historic_pricing_region.unwrap_or(Bzn::AT);

          self.historic_prices = None;
          self.historic_prices_error = None;
          self.project_path = Some(path);
          self.project_directory = Self::parent_directory(self.project_path.as_ref());
          self.needs_update = true;
          self.needs_simulation = false;
          self.power_production.clear();
          self.message = match self.save_settings() {
            Ok(()) => "Project loaded.".into(),
            Err(error) => {
              format!("Project loaded, but picker settings could not be saved: {error}")
            },
          };
        },
        Err(error) => self.message = format!("Could not load project: {error}"),
      },
      Err(error) => self.message = format!("Could not load project: {error}"),
    }

    if self.consumption_path.is_some() {
      self.load_consumption(true);
    }
    if self.consumption_path_2.is_some() {
      self.load_consumption_2(true);
    }
  }

  fn save_project(&mut self) {
    let mut dialog = rfd::FileDialog::new()
      .add_filter("Solar project", &["json"])
      .set_file_name("solar-project.json");
    if let Some(directory) = &self.project_directory {
      dialog = dialog.set_directory(directory);
    }
    let Some(path) = dialog.save_file() else {
      return;
    };

    match serde_json::to_string_pretty(&self.project()) {
      Ok(contents) => match std::fs::write(&path, contents) {
        Ok(()) => {
          self.project_path = Some(path);
          self.project_directory = Self::parent_directory(self.project_path.as_ref());
          self.message = match self.save_settings() {
            Ok(()) => "Project saved.".into(),
            Err(error) => format!("Project saved, but picker settings could not be saved: {error}"),
          };
        },
        Err(error) => self.message = format!("Could not save project: {error}"),
      },
      Err(error) => self.message = format!("Could not save project: {error}"),
    }
  }

  fn parent_directory(path: Option<&PathBuf>) -> Option<PathBuf> {
    path.and_then(|path| path.parent()).map(Path::to_path_buf)
  }

  fn save_settings(&self) -> Result<(), String> {
    let settings = Settings {
      project_directory: self.project_directory.clone(),
      project_path: self.project_path.clone(),
      consumption_directory: self.consumption_directory.clone(),
    };
    storage::save_settings(&settings)
  }

  fn save_settings_with_message(&mut self) {
    self.message = match self.save_settings() {
      Ok(()) => "Settings saved.".into(),
      Err(error) => format!("Could not save settings: {error}"),
    }
  }
}

fn pv_array_color(index: usize) -> Color32 {
  const COLORS: [Color32; 8] = [
    Color32::from_rgb(230, 159, 0),
    Color32::from_rgb(86, 180, 233),
    Color32::from_rgb(0, 158, 115),
    Color32::from_rgb(240, 228, 66),
    Color32::from_rgb(0, 114, 178),
    Color32::from_rgb(213, 94, 0),
    Color32::from_rgb(204, 121, 167),
    Color32::from_rgb(117, 112, 179),
  ];

  COLORS[index % COLORS.len()]
}

pub fn run(ignore_config: bool) -> eframe::Result {
  eframe::run_native(
    "Solar Calculator",
    eframe::NativeOptions::default(),
    Box::new(move |_| Ok(Box::new(App::default_with_config(ignore_config)))),
  )
}
