mod cli;
mod config;
mod date;
mod error;
mod holydays;
mod moon;
mod subcommands;
mod wake;
mod zodiac;

use clap::Parser;
use cli::{Cli, Command};
use config::load_config;

fn main() {
    let cli = Cli::parse();

    let config = match load_config(cli.config.as_ref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("fnord: config error: {e}");
            std::process::exit(1);
        }
    };

    let no_color = cli.no_color;
    let no_unicode = cli.no_unicode;
    let json = cli.json;

    let result = match cli
        .command
        .unwrap_or(Command::Date(cli::DateArgs::default()))
    {
        Command::Date(args) => subcommands::date::run(&args, &config, json, no_color),
        Command::Cal(args) => subcommands::cal::run(&args, &config, no_color),
        Command::Holyday(args) => subcommands::holyday::run(&args, &config, json, no_color),
        Command::Pope(args) => subcommands::pope::run(&args, &config, json, no_color, no_unicode),
        Command::Oracle(args) => {
            subcommands::oracle::run(&args, &config, json, no_color, no_unicode)
        }
        Command::Fortune(args) => {
            subcommands::fortune::run(&args, &config, json, no_color, no_unicode)
        }
        Command::Koan(args) => subcommands::koan::run(&args, &config, json, no_color, no_unicode),
        Command::Moon(args) => subcommands::moon::run(&args, &config, json, no_color, no_unicode),
        Command::Zodiac(args) => {
            subcommands::zodiac::run(&args, &config, json, no_color, no_unicode)
        }
        Command::Omens(args) => subcommands::omens::run(&args, &config, json, no_color, no_unicode),
        Command::Log(args) => subcommands::log::run(&args, &config, json, no_color),
        Command::Wake(args) => subcommands::wake::run(&args, &config, json, no_color, no_unicode),
        Command::Pineal(args) => {
            subcommands::pineal::run(&args, &config, json, no_color, no_unicode)
        }
        Command::Fnord(args) => subcommands::redact::run(&args, &config, json),
        Command::Cabbage(args) => subcommands::cabbage::run(&args, &config, json),
        Command::Chaos(args) => subcommands::chaos::run(&args, &config, json),
        Command::Law(args) => subcommands::law::run(&args, &config, json),
        Command::Pentabarf(args) => {
            subcommands::pentabarf::run(&args, &config, json, no_color, no_unicode)
        }
        Command::Hotdog(args) => subcommands::hotdog::run(&args, &config, json),
        Command::Erisian(args) => {
            subcommands::erisian::run(&args, &config, json, no_color, no_unicode)
        }
    };

    if let Err(e) = result {
        eprintln!("fnord: {e}");
        std::process::exit(1);
    }
}
