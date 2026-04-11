// Run with: cargo run --bin generate-completions --features generate-assets
use clap::CommandFactory;
use clap_complete::{generate_to, Shell};
use fn0rd_lib::cli::Cli;
use std::path::PathBuf;

fn main() {
    let mut cmd = Cli::command();
    let out_dir = PathBuf::from("completions");
    std::fs::create_dir_all(&out_dir).unwrap();

    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
        generate_to(shell, &mut cmd, "fnord", &out_dir).expect("failed to generate completions");
        println!("Generated {shell} completions → completions/");
    }

    println!("All completions generated. All Hail Eris.");
}
