mod consumption;
mod energy_charts;
mod gui;
mod model;
mod pvgis;
mod storage;

enum CliAction {
  Run(bool),
  Help,
}

fn parse_args(mut parser: lexopt::Parser) -> Result<CliAction, String> {
  let mut ignore_config = false;

  loop {
    let arg = match parser.next() {
      Ok(Some(arg)) => arg,
      Ok(None) => break,
      Err(error) => return Err(error.to_string()),
    };

    match arg {
      lexopt::Arg::Long("ignore-config") | lexopt::Arg::Long("no-config") => {
        ignore_config = true;
      },
      lexopt::Arg::Short('n') => {
        ignore_config = true;
      },
      lexopt::Arg::Long("help") | lexopt::Arg::Short('h') => {
        return Ok(CliAction::Help);
      },
      arg => return Err(format!("unexpected argument: {:?}", arg)),
    }
  }

  Ok(CliAction::Run(ignore_config))
}

fn main() {
  match parse_args(lexopt::Parser::from_env()) {
    Ok(CliAction::Run(ignore_config)) => {
      if let Err(error) = gui::run(ignore_config) {
        eprintln!("Solar Calculator failed: {error}");
        std::process::exit(1);
      }
    },
    Ok(CliAction::Help) => {
      println!("Usage: solar-calculator [--ignore-config]");
      println!("  --ignore-config, --no-config, -n  Do not load the default settings file");
    },
    Err(error) => {
      eprintln!("Error: {error}");
      eprintln!("Try 'solar-calculator --help' for usage.");
      std::process::exit(2);
    },
  }
}
