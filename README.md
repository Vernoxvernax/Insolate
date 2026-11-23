

# Insolate

Insolate is a desktop solar-energy and PV system calculator written in Rust. It estimates PV production, battery usage, grid consumption, transfer to a second location, and likely PV payback based on consumption data and system settings.

The project is mainly intended for people planning a PV system for peer-to-peer use, where surplus power can be supplied to a second house or building. A second-location consumption file is optional, so Insolate can also be used as a basic PV and battery calculator for a single property.

## Features

- Hourly PV production estimates for one or more configured PV arrays
- PV orientation and tilt configuration
- Battery size and discharge-capacity modeling
- Inverter output-capacity modeling
- Optional consumption data for a second location
- Monthly averages for battery level, grid usage, exported power, and transferred power
- Average yearly electricity costs and PV payback estimates
- Fixed or historic dynamic feed-in revenue options
- Multiple historical PV production and electricity-price years

## Requirements

- A Rust toolchain with Cargo
- A graphical desktop environment
- Internet access when loading PVGIS or energy-charts data

## Run

In the cloned repo:

```sh
cargo run --release
```

## Consumption data

At present, consumption data imports are supported only for these quarter-hourly data formats:

- Energie Steiermark
- Stromnetz Graz

Select the matching CSV format, load the CSV for the primary location, and optionally load one for the second location. The importer combines the quarter-hour entries into hourly values for the simulation.

The second location is optional. Leaving it set to `None` calculates the primary location only.

Support for additional consumption CSV formats is not included yet. If you need another provider or export format, please open an issue. Include an example CSV with personal data removed.

## Energy communities

Insolate does not currently support energy communities. Simulations are limited to one primary consumption location and one optional secondary consumption location. Modeling an energy community requires several independent secondary consumption datasets and community-specific allocation rules.

## Getting started

1. Start the application.
2. Load or enter consumption data for the primary location.
3. Optionally load consumption data for the second location.
4. Enter the PV location coordinates.
5. Select the simulation year range.
6. Add one or more PV arrays with module count, watt-peak, direction, and tilt.
7. Enter battery, inverter, and electricity-cost settings.
8. Click `Solve` to load external data and run the simulation.

Projects can be loaded and saved as JSON. Saving a project stores the system configuration and the paths to the selected consumption and project files.

## Assumptions and limitations

Insolate uses a simplified hourly simulation and makes several assumptions:

- The inverter can supply power to the second location from the battery.
- A smart meter at both premises continually measures demand and directs available PV or battery power to the location that needs it.
- Battery and inverter charge/discharge limits are modeled as the configured values.
- The main location is supplied before the secondary location.
- PV surplus is sent to the battery, then to the secondary location or public grid according to the simulation logic.
- A battery can run out of power during the night or during periods of high demand, especially when both locations are consuming electricity.
- PV arrays are modeled independently and then combined in the simulation.
- Leap days are excluded from PV and price datasets.

The inverter assumption is important: not every inverter can supply battery power to the way this model assumes. The behavior is at least applicable to Fronius inverter lines.

The smart-meter assumption is also important. If the second location has high or prolonged demand, the battery may be depleted and remaining demand will be supplied from the grid.

The results are estimates, not guarantees. Actual production and savings depend on inverter behavior, battery losses, weather, shadowing, tariffs, transfer arrangements, maintenance, and real consumption patterns.

## External services and privacy

Insolate sends only the following information to external services:

- [PVGIS](https://photovoltaic-geographic-information-system.ec.europa.eu/api/v6/docs#/Performance/get-photovoltaic-performance-analysis) receives the specified longitude and latitude, together with the PV array direction and tilt, selected year range, and system parameters.
- [Energy Charts](https://api.energy-charts.info/#/prices%20v2/handler_v2_price_get) receives the specified bidding zone/country when historic dynamic prices are enabled.

PVGIS is requested with:

- A system loss of 14 percent, configured as `system_efficiency: 0.86`
- `cSi:Integrated 2025`, the crystalline-silicon integrated-2025 photovoltaic model
- Hourly production data

Historic dynamic electricity prices are optional and can be disabled.

## Simulation output

The results include:

- Average monthly and daily PV production
- Worst and best modeled production years
- Average battery charge level
- Battery-empty periods
- Grid consumption at both locations
- Power sent to the second location
- Power sold to the grid
- Fixed or historic feed-in revenue
- Yearly electricity costs
- Estimated yearly savings and PV payback time


