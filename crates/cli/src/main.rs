mod atomic;
mod check;
mod config_cmd;
mod debug;
mod fix;
mod format;
mod hook;
mod human;
mod init;
#[cfg(feature = "pack-mode")]
mod languages_cmd;
mod lint;
#[cfg(feature = "pack-mode")]
mod missing;
mod new;
mod report;
mod rules_cmd;
mod session;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

// Measured 25-40% faster than the system allocator on the format path.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser)]
#[command(
    name = "sudsy",
    version,
    about = "Formatter and linter for source and data files"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Developer tools: parse a .cs file with the C# worker and print its tree.
    #[command(subcommand)]
    Debug(debug::DebugCommand),
    /// Format source and data files.
    Format(format::FormatArgs),
    /// Report rule violations; exit code 1 if there is an error.
    Lint(lint::LintArgs),
    /// Apply fixes (or the ones named by id), then format.
    Fix(fix::FixArgs),
    /// Run `format --check` and `lint`, for CI.
    Check(lint::LintArgs),
    /// List the rules, or explain one.
    Rules(rules_cmd::RulesArgs),
    /// Inspect the configuration.
    #[command(subcommand)]
    Config(config_cmd::ConfigCommand),
    /// Install, remove and list languages.
    #[cfg(feature = "pack-mode")]
    #[command(subcommand)]
    Languages(languages_cmd::LanguagesCommand),
    /// Write a starter sudsy.yaml.
    Init(init::InitArgs),
    /// Run the language server on stdin and stdout.
    Lsp,
    /// Answer an agent hook (Claude Code): the hook input JSON on stdin; exit 2 with the findings
    /// on stderr for the agent.
    Hook(hook::HookArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // A hook speaks the agent's exit codes, never the other commands' ones.
    let command = match cli.command {
        Command::Hook(args) => return hook::run(args),
        command => command,
    };
    let result = match command {
        Command::Debug(c) => debug::run(c).map(|()| ExitCode::SUCCESS),
        Command::Format(args) => format::run(args),
        Command::Lint(args) => lint::run(args),
        Command::Fix(args) => fix::run(args),
        Command::Check(args) => check::run(args),
        Command::Rules(args) => rules_cmd::run(args).map(|()| ExitCode::SUCCESS),
        Command::Config(c) => config_cmd::run(c).map(|()| ExitCode::SUCCESS),
        #[cfg(feature = "pack-mode")]
        Command::Languages(c) => languages_cmd::run(c),
        Command::Init(args) => init::run(args),
        Command::Lsp => lsp::run_stdio().map(|()| ExitCode::SUCCESS),
        Command::Hook(_) => unreachable!("answered above"),
    };
    match result {
        // Files were skipped for a missing language: a distinct status, unless something worse
        // already set one.
        #[cfg(feature = "pack-mode")]
        Ok(code) if code == ExitCode::SUCCESS && missing::skipped() => ExitCode::from(3),
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
