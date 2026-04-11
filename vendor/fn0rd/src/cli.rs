use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "fnord",
    about = "A Discordian calendar and chaos utility",
    long_about = "fnord — All Hail Discordia!\n\nA spiritual successor to ddate with a full suite of Discordian-themed subcommands.",
    version
)]
pub struct Cli {
    /// Path to config file (overrides default locations)
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Output JSON instead of human-readable text
    #[arg(long, global = true)]
    pub json: bool,

    /// Disable color output
    #[arg(long = "no-color", global = true)]
    pub no_color: bool,

    /// Disable unicode output (fall back to ASCII)
    #[arg(long = "no-unicode", global = true)]
    pub no_unicode: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Display today's (or a given) Discordian date
    Date(DateArgs),

    /// Display a Discordian season calendar
    Cal(CalArgs),

    /// Look up or list holydays
    Holyday(HolydayArgs),

    /// Show the current moon phase
    Moon(MoonArgs),

    /// Read the omens (weather + chaos report)
    Omens(OmensArgs),

    /// Dispense a Discordian fortune
    Fortune(FortuneArgs),

    /// Write in the Discordian grimoire (log)
    Log(LogArgs),

    /// Morning dashboard with large ASCII-art Discordian date
    Wake(WakeArgs),

    /// Display your Discordian papal credentials
    Pope(PopeArgs),

    /// Report system consciousness (pineal gland) status
    Pineal(PinealArgs),

    /// Ask the oracle a question
    Oracle(OracleArgs),

    /// Apply fnord redaction to text
    Fnord(FnordRedactArgs),

    /// Determine whether a file is a hotdog
    Hotdog(HotdogArgs),

    /// Count content in Discordian units
    Cabbage(CabbageArgs),

    /// Shuffle lines, words, or characters
    Chaos(ChaosArgs),

    /// Search text and apply the Law of Fives
    Law(LawArgs),

    /// Validate text against the Five Commandments
    Pentabarf(PentabarfArgs),

    /// Diff two files as a theological dispute
    Erisian(ErisianArgs),

    /// Dispense a Zen koan (Discordian edition)
    Koan(KoanArgs),

    /// Display your zodiac sign
    Zodiac(ZodiacArgs),
}

#[derive(Args, Debug, Default)]
pub struct DateArgs {
    /// Date to display (today, yesterday, tomorrow, YYYY-MM-DD, +N, -N)
    #[arg(long, short = 'd', value_name = "DATE")]
    pub date: Option<String>,

    /// Show only the date line (no holyday info)
    #[arg(long, short = 's')]
    pub short: bool,

    /// Include apostle information
    #[arg(long, short = 'a')]
    pub apostle: bool,

    /// Include holyday information if applicable
    #[arg(long, short = 'H')]
    pub holydays: bool,

    /// Custom format string (%A weekday, %B season, %d day, %e ordinal day, %Y year, %H holyday, %a apostle, %n newline, %t tab)
    #[arg(long, short = 'f', value_name = "FORMAT")]
    pub format: Option<String>,

    /// Print a reference table of all format tokens and exit
    #[arg(long)]
    pub help_format: bool,
}

#[derive(Args, Debug, Default)]
pub struct CalArgs {
    /// Season to display (chaos, discord, confusion, bureaucracy, aftermath)
    #[arg(long, short = 's', value_name = "SEASON")]
    pub season: Option<String>,

    /// Year (YOLD) to display
    #[arg(long, short = 'y', value_name = "YEAR")]
    pub year: Option<i32>,

    /// Display all 5 seasons
    #[arg(long, short = 'a')]
    pub all: bool,

    /// Terminal width for responsive layout
    #[arg(long, short = 'w', value_name = "WIDTH")]
    pub width: Option<usize>,
}

#[derive(Args, Debug, Default)]
pub struct HolydayArgs {
    #[command(subcommand)]
    pub action: Option<HolydayAction>,
}

#[derive(Subcommand, Debug)]
pub enum HolydayAction {
    /// List all known holydays
    List(HolydayListArgs),
    /// Show holyday for a given date (or today)
    Show(HolydayShowArgs),
    /// Add a holyday to the personal holyday file
    Add(HolydayAddArgs),
    /// Remove a holyday from the personal holyday file
    Remove(HolydayRemoveArgs),
}

#[derive(Args, Debug, Default)]
pub struct HolydayListArgs {
    /// Filter by season (chaos, discord, confusion, bureaucracy, aftermath)
    #[arg(long, value_name = "SEASON")]
    pub season: Option<String>,

    /// Show source tags [default], [cabal], [personal]
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub show_source: bool,

    /// Output as JSON array
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Default)]
pub struct HolydayShowArgs {
    /// Date to check (today, yesterday, tomorrow, YYYY-MM-DD, +N, -N)
    #[arg(value_name = "DATE")]
    pub date: Option<String>,
}

#[derive(Args, Debug)]
pub struct HolydayAddArgs {
    /// Date key (e.g. chaos-15, st-tibs)
    #[arg(value_name = "KEY")]
    pub key: String,

    /// Holyday name
    #[arg(value_name = "NAME")]
    pub name: String,

    /// Description of the holyday
    #[arg(long)]
    pub description: Option<String>,

    /// One-time holyday (requires --year)
    #[arg(long)]
    pub once: bool,

    /// Year for a one-time holyday (YOLD)
    #[arg(long, value_name = "YEAR")]
    pub year: Option<i32>,
}

#[derive(Args, Debug)]
pub struct HolydayRemoveArgs {
    /// Date key to remove (e.g. chaos-15)
    #[arg(value_name = "KEY")]
    pub key: String,
}

/// Stub args for unimplemented subcommands
#[derive(Args, Debug, Default)]
pub struct StubArgs {}

#[derive(Args, Debug, Default)]
pub struct FnordRedactArgs {
    /// Input file (reads from stdin if omitted)
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Replacement rate, 0.0 to 1.0 (overrides config)
    #[arg(long, short = 'r', value_name = "RATE")]
    pub rate: Option<f64>,

    /// Seed string for reproducible chaos
    #[arg(long, short = 's', value_name = "SEED")]
    pub seed: Option<String>,

    /// Disable preserve_structure — every word eligible
    #[arg(long = "pure-chaos")]
    pub pure_chaos: bool,

    /// Override the replacement string (default "FNORD")
    #[arg(long, value_name = "WORD")]
    pub replacement: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct CabbageArgs {
    /// One or more input files (reads from stdin if none)
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    /// Show only cabbage count
    #[arg(long, short = 'c')]
    pub cabbages: bool,

    /// Show only Discord Unit count
    #[arg(long = "discord-units", short = 'd')]
    pub discord_units: bool,

    /// Show only Erg of Confusion count
    #[arg(long, short = 'e')]
    pub ergs: bool,
}

#[derive(Args, Debug, Default)]
pub struct ChaosArgs {
    /// Input file (reads from stdin if omitted)
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Shuffle words within each line (instead of lines)
    #[arg(long, short = 'w', conflicts_with = "chars")]
    pub words: bool,

    /// Shuffle all characters globally (preserving newlines)
    #[arg(long, short = 'c')]
    pub chars: bool,

    /// Seed string for reproducible chaos
    #[arg(long, short = 's', value_name = "SEED")]
    pub seed: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct LawArgs {
    /// Pattern to search for
    #[arg(value_name = "PATTERN")]
    pub pattern: String,

    /// Files to search (reads stdin if none)
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    /// Case-insensitive matching
    #[arg(long = "ignore-case", short = 'i')]
    pub ignore_case: bool,

    /// Match whole words only
    #[arg(long, short = 'w')]
    pub word: bool,

    /// Invert match (lines that do NOT match)
    #[arg(long, short = 'v')]
    pub invert: bool,

    /// Suppress the Law of Fives analysis
    #[arg(long = "no-law")]
    pub no_law: bool,
}

#[derive(Args, Debug, Default)]
pub struct PentabarfArgs {
    /// Input file (reads from stdin if omitted)
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Exit with code = 10 - score (for CI pipelines)
    #[arg(long)]
    pub strict: bool,
}

#[derive(Args, Debug, Default)]
pub struct HotdogArgs {
    /// One or more files to classify
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    /// Brief output (one line per file)
    #[arg(long, short = 'b')]
    pub brief: bool,

    /// Suppress justification text
    #[arg(long = "no-justify")]
    pub no_justify: bool,
}

#[derive(Args, Debug, Default)]
pub struct ErisianArgs {
    /// File A (ORDER)
    #[arg(value_name = "FILE_A")]
    pub file_a: PathBuf,

    /// File B (CHAOS)
    #[arg(value_name = "FILE_B")]
    pub file_b: PathBuf,

    /// Show only the theological summary
    #[arg(long, short = 's')]
    pub summary: bool,

    /// Number of context lines around each dispute
    #[arg(long, short = 'C', value_name = "N", default_value_t = 3)]
    pub context: usize,
}

#[derive(Args, Debug, Default)]
pub struct PopeArgs {
    /// Print a single-line summary instead of the full declaration
    #[arg(long, short = 's')]
    pub short: bool,

    /// Emit a full Papal Bull (multi-line ASCII document)
    #[arg(long, short = 'b')]
    pub bull: bool,

    /// Regenerate the papal identity (non-deterministic for this run)
    #[arg(long, short = 'r')]
    pub reroll: bool,
}

#[derive(Args, Debug, Default)]
pub struct OracleArgs {
    /// The question to ask the Oracle. If omitted, you will be prompted.
    #[arg(value_name = "QUESTION")]
    pub question: Option<String>,

    /// Reveal the raw seed value used to compute the answer
    #[arg(long = "reveal-seed")]
    pub reveal_seed: bool,

    /// Mix in the current timestamp to produce a non-deterministic answer
    #[arg(long)]
    pub chaos: bool,
}

#[derive(Args, Debug, Default)]
pub struct FortuneArgs {
    /// Number of fortunes to print, separated by %
    #[arg(long, short = 'c', default_value_t = 1)]
    pub count: usize,

    /// Filter to fortunes with a matching tag (e.g. "chaos" or "season:chaos")
    #[arg(long, short = 't', value_name = "TAG")]
    pub tag: Option<String>,

    /// Ignore all weighting and pick fortunes uniformly at random
    #[arg(long, short = 'r')]
    pub random: bool,

    /// Include fortunes from the offensive corpus if configured
    #[arg(long)]
    pub offensive: bool,
}

#[derive(Args, Debug, Default)]
pub struct MoonArgs {
    /// Override the celestial body from config (luna, phobos, deimos, io,
    /// europa, ganymede, titan, triton, random)
    #[arg(long, short = 'b', value_name = "BODY")]
    pub body: Option<String>,

    /// Date to display the phase for (today, yesterday, tomorrow, YYYY-MM-DD, +N, -N)
    #[arg(long, short = 'd', value_name = "DATE")]
    pub date: Option<String>,

    /// Append next full moon and next new moon in Discordian form
    #[arg(long, short = 'n')]
    pub next: bool,

    /// Render a table of phases for every 5th day of the current Discordian season
    #[arg(long, short = 's')]
    pub season: bool,
}

#[derive(Args, Debug, Default)]
pub struct ZodiacArgs {
    /// Zodiac system (western, vedic, chinese, discordian)
    #[arg(long, short = 's', value_name = "SYSTEM")]
    pub system: Option<String>,

    /// Date to use for the lookup (today, yesterday, tomorrow, YYYY-MM-DD, +N, -N)
    #[arg(long, short = 'd', value_name = "DATE")]
    pub date: Option<String>,

    /// Show the extended description of the sign
    #[arg(long, short = 'f')]
    pub full: bool,
}

#[derive(Args, Debug, Default)]
pub struct OmensArgs {
    /// Location override (defaults to config.weather.location)
    #[arg(long, short = 'l', value_name = "LOCATION")]
    pub location: Option<String>,

    /// Force generative mode even if location is configured and network works
    #[arg(long, short = 'g')]
    pub generative: bool,

    /// Append the raw metric weather values below the omen output
    #[arg(long, short = 'r')]
    pub raw: bool,

    /// Units override ("discordian" or "metric")
    #[arg(long, short = 'u', value_name = "UNITS")]
    pub units: Option<String>,

    /// Date to read the omens for (today, yesterday, tomorrow, YYYY-MM-DD, +N, -N)
    #[arg(long, short = 'd', value_name = "DATE")]
    pub date: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct LogArgs {
    /// Entry text. If omitted, launches $EDITOR with a temp file.
    #[arg(value_name = "MESSAGE")]
    pub message: Option<String>,

    /// Override the grimoire path for this invocation only
    #[arg(long, short = 'F', value_name = "FILE")]
    pub file: Option<String>,

    /// Append a fortune after the entry body
    #[arg(long)]
    pub fortune: bool,

    /// Append today's omens after the entry body (generative)
    #[arg(long)]
    pub omens: bool,

    /// Display the last N entries in reverse chronological order
    #[arg(long, value_name = "N", num_args = 0..=1, default_missing_value = "10")]
    pub list: Option<usize>,

    /// Override the entry format (plaintext, markdown, org)
    #[arg(long, value_name = "FORMAT")]
    pub format: Option<String>,

    /// Override the timestamp style (discordian, iso8601, both)
    #[arg(long = "timestamp-style", value_name = "STYLE")]
    pub timestamp_style: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct WakeArgs {
    /// Suppress the moon panel
    #[arg(long = "no-moon")]
    pub no_moon: bool,

    /// Show the omens panel
    #[arg(long)]
    pub omens: bool,

    /// Show the fortune panel
    #[arg(long)]
    pub fortune: bool,

    /// ASCII art font (standard, banner, doom, smush)
    #[arg(long, value_name = "FONT")]
    pub font: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct PinealArgs {
    /// Verbosity level (minimal, normal, enlightened)
    #[arg(long, short = 'v', value_name = "LEVEL")]
    pub verbosity: Option<String>,

    /// Append raw system values below the styled output
    #[arg(long, short = 'r')]
    pub raw: bool,
}

#[derive(Args, Debug, Default)]
pub struct KoanArgs {
    /// Number of koans to generate, separated by a blank line
    #[arg(long, short = 'c', default_value_t = 1)]
    pub count: usize,

    /// Reproducible seed (any string); same seed always yields the same koan
    #[arg(long, short = 's', value_name = "SEED")]
    pub seed: Option<String>,
}
