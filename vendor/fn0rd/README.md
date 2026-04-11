# fnord

> A Discordian calendar and chaos utility.
> Contains 5 tons of flax.

[![Crates.io](https://img.shields.io/crates/v/fn0rd.svg)](https://crates.io/crates/fn0rd)
[![License: WTFPL](https://img.shields.io/badge/license-WTFPL-brightgreen.svg)](LICENSE)

All Hail Eris. All Hail Discordia.

A Rust reimagining of the classic `ddate` utility, extended with a full suite of
Discordian subcommands for those who know, in their hearts, that everything is
related to the number five.

---

## Installation

```bash
cargo install fn0rd
```

Or from source:

```bash
git clone https://github.com/maravexa/fnord
cd fnord
cargo install --path .
```

---

## Quick start

```bash
fnord              # today's Discordian date (replaces ddate)
fnord pope         # discover your papal identity
fnord cal          # current season calendar
fnord oracle "is a hotdog a sandwich"
```

Example output:

```
Today is Prickle-Prickle, the 16th of Chaos, in the YOLD 3191
Apostle: Hung Mung
```

---

## Subcommands

| Command       | Description                                                   |
|---------------|---------------------------------------------------------------|
| `date`        | Display today's (or a given) Discordian date                  |
| `cal`         | Display a Discordian season calendar                          |
| `holyday`     | List, show, add, or remove holydays                           |
| `moon`        | Show the current moon phase (supports 8 celestial bodies)     |
| `omens`       | Read the omens — weather + chaos report                       |
| `fortune`     | Dispense a Discordian fortune                                 |
| `log`         | Write in the Discordian grimoire                              |
| `wake`        | Morning dashboard with large ASCII-art Discordian date        |
| `pope`        | Display your Discordian papal credentials                     |
| `pineal`      | Report system consciousness (pineal gland) status             |
| `oracle`      | Ask the oracle a question                                     |
| `koan`        | Dispense a Zen koan (Discordian edition)                      |
| `zodiac`      | Display your zodiac sign (western, vedic, chinese, discordian)|
| `fnord`       | Apply fnord redaction to text                                 |
| `cabbage`     | Count content in Discordian units                             |
| `chaos`       | Shuffle lines, words, or characters                           |
| `law`         | Search text and apply the Law of Fives                        |
| `pentabarf`   | Validate text against the Five Commandments                   |
| `hotdog`      | Determine whether a file is a hotdog                          |
| `erisian`     | Diff two files as a theological dispute                       |

---

## `fnord date` usage

```bash
fnord date                                     # today's date
fnord date --date 2025-01-05                   # specific date (ISO 8601)
fnord date --date tomorrow                     # relative: today, yesterday, tomorrow
fnord date --date +7                           # 7 days from now
fnord date --short                             # date line only
fnord date --apostle                           # include apostle name
fnord date --holydays                          # include holyday info
fnord date --format "%A, the %e of %B, YOLD %Y"  # custom format string
fnord date --help-format                       # show format token reference
fnord date --json                              # JSON output
```

### Format tokens

| Token | Meaning                          | Example         |
|-------|----------------------------------|-----------------|
| `%A`  | Weekday name                     | `Pungenday`     |
| `%B`  | Season name                      | `Confusion`     |
| `%d`  | Day of season (numeric)          | `23`            |
| `%e`  | Day of season (ordinal)          | `23rd`          |
| `%Y`  | YOLD year                        | `3192`          |
| `%H`  | Holyday name (empty if none)     | `Confuflux`     |
| `%a`  | Apostle name                     | `Sri Syadasti`  |
| `%n`  | Newline                          |                 |
| `%t`  | Tab                              |                 |
| `%%`  | Literal percent sign             | `%`             |

### ddate compatibility

To reproduce classic `ddate` output:

```bash
fnord date --format "%{%A, %e day of %B%}, YOLD %Y%N%nCelebrate %H"
# or simply:
fnord date
```

The classic `ddate` long format is equivalent to:

```bash
fnord date --format "%A, the %e of %B, in the YOLD %Y"
```

---

## `fnord cal` usage

```bash
fnord cal                    # current season
fnord cal --season discord   # specific season
fnord cal --year 3191        # specific year
fnord cal --all              # all 5 seasons
fnord cal --no-color         # disable ANSI colors
```

Example output:

```
  ╔══════════════════════════════════════╗
  ║     SEASON OF CHAOS — YOLD 3191      ║
  ╚══════════════════════════════════════╝

  SM   BT   PG   PP   SO
   1    2    3    4    5 ★
   6    7    8    9   10
  ...
```

---

## `fnord holyday` usage

```bash
fnord holyday list                          # list all holydays
fnord holyday list --season chaos           # filter by season
fnord holyday list --json                   # JSON output
fnord holyday show                          # holyday for today
fnord holyday show 2025-01-05              # holyday for a specific date
fnord holyday add chaos-15 "The Incident" --description "We do not speak of it"
fnord holyday add discord-5 "A One-Time Thing" --once --year 3192
fnord holyday remove chaos-15
```

---

## `fnord pope` usage

```bash
fnord pope           # full papal declaration
fnord pope --short   # one-line summary
fnord pope --bull    # full Papal Bull document
fnord pope --json    # JSON output
```

Example output:

```
╔══════════════════════════════════════════════╗
║           PAPAL DECLARATION                  ║
╚══════════════════════════════════════════════╝

  By the authority vested in me by Eris, Goddess of Discord,
  I hereby declare:

  eris@archbox is ordained as
  Pope Eris the Magnificent of the Holy Order of the Golden Apple
  ...
```

---

## `fnord moon` usage

```bash
fnord moon                     # current moon phase (Luna)
fnord moon --body phobos       # Phobos phase
fnord moon --body random       # random body
fnord moon --next              # next full and new moon
fnord moon --season            # season phase table
fnord moon --json              # JSON output
```

### Supported celestial bodies

| Body      | Synodic period |
|-----------|----------------|
| `luna`    | 29.53 days     |
| `phobos`  | 0.319 days     |
| `deimos`  | 1.263 days     |
| `io`      | 1.769 days     |
| `europa`  | 3.551 days     |
| `ganymede`| 7.155 days     |
| `titan`   | 15.945 days    |
| `triton`  | 5.877 days     |

---

## Configuration

Location: `~/.config/eris/fnord.toml`

```toml
[identity]
pope_title = "Pope Awesome McSomeone I"
sect_name  = "The Wholly Confused"
cabal      = "POEE"

[calendar]
show_apostle = true
show_holyday = true

[weather]
location = "San Francisco, CA"
units    = "discordian"

[moon]
body = "luna"

[zodiac]
system = "discordian"

[fortune]
count = 1

[log]
path   = "~/.local/share/eris/grimoire.md"
format = "markdown"

[output]
color   = "auto"   # "auto", "always", "never"
unicode = true
```

Config is loaded in order (later layers override earlier):

1. `/etc/eris/fnord.toml` — system-wide
2. `~/.config/eris/fnord.toml` — user config
3. `$FNORD_CONFIG` — env var path override
4. `--config <file>` — CLI flag override

---

## Custom holy days

Personal holydays live in `~/.config/eris/holydays/personal.toml`.

Add them via CLI:

```bash
fnord holyday add chaos-15 "The Incident" --description "We do not speak of it"
```

Or edit the file directly:

```toml
[[holyday]]
name        = "The Incident"
date        = "chaos-15"
description = "We do not speak of it"
recurring   = true

[[holyday]]
name        = "A One-Time Thing"
date        = "discord-5"
year        = 3192
recurring   = false
```

Valid `date` values: `st-tibs`, `chaos-5`, `discord-50`, `confusion-23`, etc.

Personal holydays override default and cabal holydays for the same date.

---

## Global flags

All subcommands accept these global flags:

| Flag          | Description                              |
|---------------|------------------------------------------|
| `--json`      | Output JSON instead of human-readable    |
| `--no-color`  | Disable ANSI color output                |
| `--no-unicode`| Fall back to ASCII (no box-drawing chars)|
| `--config`    | Override config file path                |

---

## Generating shell completions

```bash
cargo run --bin generate-completions --features generate-assets
```

Completions are written to `completions/` for Bash, Zsh, Fish, and PowerShell.

Install Bash completions:

```bash
source completions/fnord.bash
```

---

## Generating man pages

```bash
cargo build --features generate-assets
```

Man pages are written to `man/`. Install with:

```bash
sudo cp man/fnord.1 /usr/local/share/man/man1/
sudo cp man/fnord-*.1 /usr/local/share/man/man1/
```

---

## License

WTFPL — see [LICENSE](LICENSE).

```
        DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
                Version 2, December 2004

You just DO WHAT THE FUCK YOU WANT TO.
```

*All Hail Eris. All Hail Discordia. Contains 5 tons of flax.*
