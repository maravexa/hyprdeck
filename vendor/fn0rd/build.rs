fn main() {
    // Only generate assets when the feature is explicitly enabled.
    // This keeps normal builds fast.
    #[cfg(feature = "generate-assets")]
    generate_assets();
}

#[cfg(feature = "generate-assets")]
fn generate_assets() {
    use clap::CommandFactory;
    use clap_mangen::Man;
    use std::path::PathBuf;

    // build.rs cannot import fnord_lib (circular dep), so we re-create
    // the command structure here using clap's builder API.
    let cmd = build_command();

    let man_dir = PathBuf::from("man");
    std::fs::create_dir_all(&man_dir).unwrap();

    // Top-level man page
    write_man(&Man::new(cmd.clone()), &man_dir, "fnord.1");

    // Subcommand man pages
    for sub in cmd.get_subcommands() {
        let name = format!("fnord-{}", sub.get_name());
        let man = Man::new(sub.clone().name(&name));
        write_man(&man, &man_dir, &format!("{name}.1"));
    }

    println!("cargo:rerun-if-changed=src/cli.rs");
}

#[cfg(feature = "generate-assets")]
fn write_man(man: &clap_mangen::Man, dir: &std::path::Path, filename: &str) {
    let path = dir.join(filename);
    let mut buf = Vec::new();
    man.render(&mut buf).expect("failed to render man page");
    std::fs::write(&path, buf).expect("failed to write man page");
    eprintln!("Generated {}", path.display());
}

#[cfg(feature = "generate-assets")]
fn build_command() -> clap::Command {
    use clap::{Arg, ArgAction, Command};

    Command::new("fnord")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Your Name <you@example.com>")
        .about("A Discordian calendar and chaos utility")
        .long_about(
            "fnord — All Hail Discordia!\n\n\
             A spiritual successor to ddate with a full suite of Discordian-themed subcommands.\n\n\
             NAME\n    fnord - A Discordian calendar and chaos utility\n\n\
             DESCRIPTION\n    All Hail Eris. All Hail Discordia.\n    fnord weighs exactly 5 tons of flax.",
        )
        .arg(Arg::new("config").long("config").global(true).help("Path to config file"))
        .arg(Arg::new("json").long("json").global(true).action(ArgAction::SetTrue).help("Output JSON"))
        .arg(Arg::new("no-color").long("no-color").global(true).action(ArgAction::SetTrue).help("Disable color"))
        .arg(Arg::new("no-unicode").long("no-unicode").global(true).action(ArgAction::SetTrue).help("Disable unicode"))
        .subcommand(
            Command::new("date")
                .about("Display today's (or a given) Discordian date")
                .arg(Arg::new("date").long("date").short('d').help("Date to display"))
                .arg(Arg::new("short").long("short").short('s').action(ArgAction::SetTrue).help("Short output"))
                .arg(Arg::new("apostle").long("apostle").short('a').action(ArgAction::SetTrue).help("Include apostle"))
                .arg(Arg::new("holydays").long("holydays").short('H').action(ArgAction::SetTrue).help("Include holyday info"))
                .arg(Arg::new("format").long("format").short('f').help("Custom format string"))
                .arg(Arg::new("help-format").long("help-format").action(ArgAction::SetTrue).help("Show format token reference")),
        )
        .subcommand(
            Command::new("cal")
                .about("Display a Discordian season calendar")
                .arg(Arg::new("season").long("season").short('s').help("Season to display"))
                .arg(Arg::new("year").long("year").short('y').help("Year (YOLD)"))
                .arg(Arg::new("all").long("all").short('a').action(ArgAction::SetTrue).help("All 5 seasons")),
        )
        .subcommand(
            Command::new("holyday")
                .about("Look up, list, add, or remove holydays")
                .subcommand(Command::new("list").about("List all holydays")
                    .arg(Arg::new("season").long("season").help("Filter by season"))
                    .arg(Arg::new("json").long("json").action(ArgAction::SetTrue).help("JSON output")))
                .subcommand(Command::new("show").about("Show holyday for a date")
                    .arg(Arg::new("date").help("Date to check")))
                .subcommand(Command::new("add").about("Add a personal holyday")
                    .arg(Arg::new("key").required(true).help("Date key (e.g. chaos-15)"))
                    .arg(Arg::new("name").required(true).help("Holyday name"))
                    .arg(Arg::new("description").long("description").help("Description"))
                    .arg(Arg::new("once").long("once").action(ArgAction::SetTrue).help("One-time only"))
                    .arg(Arg::new("year").long("year").help("Year (required with --once)")))
                .subcommand(Command::new("remove").about("Remove a personal holyday")
                    .arg(Arg::new("key").required(true).help("Date key to remove"))),
        )
        .subcommand(
            Command::new("moon")
                .about("Show the current moon phase")
                .arg(Arg::new("body").long("body").short('b').help("Celestial body"))
                .arg(Arg::new("date").long("date").short('d').help("Date"))
                .arg(Arg::new("next").long("next").short('n').action(ArgAction::SetTrue).help("Next full/new moon"))
                .arg(Arg::new("season").long("season").short('s').action(ArgAction::SetTrue).help("Season table")),
        )
        .subcommand(Command::new("omens").about("Read the omens (weather + chaos report)")
            .arg(Arg::new("location").long("location").short('l').help("Location override"))
            .arg(Arg::new("generative").long("generative").short('g').action(ArgAction::SetTrue).help("Force generative mode"))
            .arg(Arg::new("raw").long("raw").short('r').action(ArgAction::SetTrue).help("Append raw values"))
            .arg(Arg::new("units").long("units").short('u').help("Units override"))
            .arg(Arg::new("date").long("date").short('d').help("Date")))
        .subcommand(Command::new("fortune").about("Dispense a Discordian fortune")
            .arg(Arg::new("count").long("count").short('c').help("Number of fortunes"))
            .arg(Arg::new("tag").long("tag").short('t').help("Filter by tag"))
            .arg(Arg::new("random").long("random").short('r').action(ArgAction::SetTrue).help("Uniform random"))
            .arg(Arg::new("offensive").long("offensive").action(ArgAction::SetTrue).help("Include offensive corpus")))
        .subcommand(Command::new("log").about("Write in the Discordian grimoire")
            .arg(Arg::new("message").help("Entry text"))
            .arg(Arg::new("file").long("file").short('F').help("Grimoire path"))
            .arg(Arg::new("list").long("list").help("Display last N entries")))
        .subcommand(Command::new("wake").about("Morning dashboard with ASCII-art Discordian date")
            .arg(Arg::new("no-moon").long("no-moon").action(ArgAction::SetTrue).help("Suppress moon panel"))
            .arg(Arg::new("omens").long("omens").action(ArgAction::SetTrue).help("Show omens panel"))
            .arg(Arg::new("fortune").long("fortune").action(ArgAction::SetTrue).help("Show fortune panel"))
            .arg(Arg::new("font").long("font").help("ASCII art font")))
        .subcommand(Command::new("pope").about("Display your Discordian papal credentials")
            .arg(Arg::new("short").long("short").short('s').action(ArgAction::SetTrue).help("Short summary"))
            .arg(Arg::new("bull").long("bull").short('b').action(ArgAction::SetTrue).help("Full Papal Bull"))
            .arg(Arg::new("reroll").long("reroll").short('r').action(ArgAction::SetTrue).help("Regenerate identity")))
        .subcommand(Command::new("pineal").about("Report system consciousness status")
            .arg(Arg::new("verbosity").long("verbosity").short('v').help("Verbosity level"))
            .arg(Arg::new("raw").long("raw").short('r').action(ArgAction::SetTrue).help("Append raw values")))
        .subcommand(Command::new("oracle").about("Ask the oracle a question")
            .arg(Arg::new("question").help("The question to ask"))
            .arg(Arg::new("reveal-seed").long("reveal-seed").action(ArgAction::SetTrue).help("Reveal seed"))
            .arg(Arg::new("chaos").long("chaos").action(ArgAction::SetTrue).help("Non-deterministic")))
        .subcommand(Command::new("fnord").about("Apply fnord redaction to text")
            .arg(Arg::new("file").help("Input file"))
            .arg(Arg::new("rate").long("rate").short('r').help("Replacement rate"))
            .arg(Arg::new("seed").long("seed").short('s').help("Seed string"))
            .arg(Arg::new("pure-chaos").long("pure-chaos").action(ArgAction::SetTrue).help("Pure chaos mode"))
            .arg(Arg::new("replacement").long("replacement").help("Replacement word")))
        .subcommand(Command::new("cabbage").about("Count content in Discordian units")
            .arg(Arg::new("files").help("Input files").num_args(0..))
            .arg(Arg::new("cabbages").long("cabbages").short('c').action(ArgAction::SetTrue).help("Cabbage count only"))
            .arg(Arg::new("discord-units").long("discord-units").short('d').action(ArgAction::SetTrue).help("Discord Units only"))
            .arg(Arg::new("ergs").long("ergs").short('e').action(ArgAction::SetTrue).help("Ergs only")))
        .subcommand(Command::new("chaos").about("Shuffle lines, words, or characters")
            .arg(Arg::new("file").help("Input file"))
            .arg(Arg::new("words").long("words").short('w').action(ArgAction::SetTrue).help("Shuffle words"))
            .arg(Arg::new("chars").long("chars").short('c').action(ArgAction::SetTrue).help("Shuffle characters"))
            .arg(Arg::new("seed").long("seed").short('s').help("Seed string")))
        .subcommand(Command::new("law").about("Search text and apply the Law of Fives")
            .arg(Arg::new("pattern").required(true).help("Pattern to search for"))
            .arg(Arg::new("files").help("Files to search").num_args(0..))
            .arg(Arg::new("ignore-case").long("ignore-case").short('i').action(ArgAction::SetTrue).help("Case-insensitive"))
            .arg(Arg::new("word").long("word").short('w').action(ArgAction::SetTrue).help("Whole words only"))
            .arg(Arg::new("invert").long("invert").short('v').action(ArgAction::SetTrue).help("Invert match"))
            .arg(Arg::new("no-law").long("no-law").action(ArgAction::SetTrue).help("Suppress Law of Fives")))
        .subcommand(Command::new("pentabarf").about("Validate text against the Five Commandments")
            .arg(Arg::new("file").help("Input file"))
            .arg(Arg::new("strict").long("strict").action(ArgAction::SetTrue).help("Strict mode")))
        .subcommand(Command::new("hotdog").about("Determine whether a file is a hotdog")
            .arg(Arg::new("files").required(true).help("Files to classify").num_args(1..))
            .arg(Arg::new("brief").long("brief").short('b').action(ArgAction::SetTrue).help("Brief output"))
            .arg(Arg::new("no-justify").long("no-justify").action(ArgAction::SetTrue).help("Suppress justification")))
        .subcommand(Command::new("erisian").about("Diff two files as a theological dispute")
            .arg(Arg::new("file_a").required(true).help("File A (ORDER)"))
            .arg(Arg::new("file_b").required(true).help("File B (CHAOS)"))
            .arg(Arg::new("summary").long("summary").short('s').action(ArgAction::SetTrue).help("Summary only"))
            .arg(Arg::new("context").long("context").short('C').help("Context lines")))
        .subcommand(Command::new("koan").about("Dispense a Zen koan (Discordian edition)")
            .arg(Arg::new("count").long("count").short('c').help("Number of koans"))
            .arg(Arg::new("seed").long("seed").short('s').help("Reproducible seed")))
        .subcommand(Command::new("zodiac").about("Display your zodiac sign")
            .arg(Arg::new("system").long("system").short('s').help("Zodiac system"))
            .arg(Arg::new("date").long("date").short('d').help("Date"))
            .arg(Arg::new("full").long("full").short('f').action(ArgAction::SetTrue).help("Extended description")))
}
