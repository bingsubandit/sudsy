//! `sudsy init`: writes a starter `sudsy.yaml`; in a terminal it asks first (languages, style,
//! tools to migrate, lint level, agents, editor, CI), with `--from <tool>` it writes the
//! equivalent of another tool's config in the working directory (docs/ai/config/cli.md).
//! Questions and flags fill one `Answers` struct; `plan` turns it into files.

mod agents;
mod ask;
mod biome;
mod csharpier;
mod editor;
mod editorconfig;
mod eslint;
mod hooks;
mod html;
mod json_edit;
mod out;
mod plan;
mod prettier;
mod prompt;
mod sources;
mod stylecop;
mod stylelint;
mod world;
mod yamllint;

#[cfg(test)]
mod tests;

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use languages::providers;
use out::{Out, Report};
use plan::{Action, Answers, Plan};
use prompt::{Cancelled, Prompter};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use world::{Found, World};

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Tool {
    Yamllint,
    Prettier,
    Biome,
    #[value(alias = "eslint-jsonc")]
    Eslint,
    Stylecop,
    Stylelint,
    Csharpier,
    HtmlValidate,
    Htmlhint,
    HtmlEslint,
}

/// A tool's config is not in the working directory (any other error means it is there and
/// cannot be used).
#[derive(Debug)]
struct NotFound(String);

impl std::fmt::Display for NotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NotFound {}

#[derive(Args)]
pub struct InitArgs {
    /// A preset to extend, e.g. `python/ruff`; repeat for several languages. Without it the
    /// starter extends the default preset of each language.
    #[arg(long = "preset")]
    presets: Vec<String>,
    /// Migrate the config of another tool found in the working directory; repeat for several.
    #[arg(long)]
    from: Vec<Tool>,
    /// Replace an existing sudsy config.
    #[arg(long)]
    force: bool,
    /// With `--from stylecop` or `csharpier`: append the `.editorconfig` entries to the file.
    #[arg(long, requires = "from")]
    write_editorconfig: bool,
    /// Add a block about `sudsy check` and `sudsy fix` to `AGENTS.md` (`CLAUDE.md` when only that
    /// exists). Alone it writes nothing else; once only.
    #[arg(long)]
    agents: bool,
    /// Answer every question with its default, also without a terminal.
    #[arg(long, short = 'y')]
    yes: bool,
    /// Set up only these languages (names as in `sudsy.yaml`); the others found are excluded.
    #[arg(long, value_delimiter = ',')]
    languages: Vec<String>,
    /// Recommend the VS Code extension and format on save for the languages set up.
    #[arg(long)]
    vscode: bool,
    /// Write `.github/workflows/sudsy.yml`, a job that runs `sudsy check`.
    #[arg(long)]
    ci: bool,
}

/// Questions are asked only when a person is there: both ends are terminals and no CI variable
/// is set.
fn asks_questions() -> bool {
    const CI: [&str; 6] = [
        "CI",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "TF_BUILD",
        "JENKINS_URL",
        "BUILDKITE",
    ];
    std::io::stdin().is_terminal()
        && std::io::stderr().is_terminal()
        && !CI.iter().any(|v| std::env::var_os(v).is_some())
}

pub fn run(args: InitArgs) -> Result<ExitCode> {
    let only_agents = args.agents
        && args.presets.is_empty()
        && args.from.is_empty()
        && args.languages.is_empty()
        && !(args.force || args.yes || args.vscode || args.ci);
    if only_agents {
        add_agents()?;
        return Ok(ExitCode::SUCCESS);
    }
    let agents = args.agents;
    let flow = args.yes || asks_questions();
    let result = if flow { interactive(args) } else { init(args) };
    match result {
        Ok(()) => {}
        Err(e) if e.downcast_ref::<Cancelled>().is_some() => {
            eprintln!("cancelled; nothing written");
            return Ok(ExitCode::from(1));
        }
        Err(e) => return Err(e),
    }
    if agents && !flow {
        add_agents()?;
    }
    Ok(ExitCode::SUCCESS)
}

/// The agent block and the Claude Code hooks.
fn add_agents() -> Result<()> {
    let dir = Path::new(".");
    match agents::add(dir)? {
        (name, true) => eprintln!("added the Sudsy block to {name}"),
        (name, false) => eprintln!("{name} already has the Sudsy block"),
    }
    let mut plan = Plan::default();
    hooks::add(dir, &mut plan);
    write(&plan, dir)?;
    if !plan.files.is_empty() {
        eprintln!("added the Sudsy hooks to {}", hooks::SETTINGS);
    }
    for note in &plan.notes {
        eprintln!("{note}");
    }
    Ok(())
}

/// The config file `init` writes, and whether it may replace one that is there.
fn config_path(force: bool) -> Result<PathBuf> {
    let existing = ["sudsy.yaml", "sudsy.yml"]
        .map(PathBuf::from)
        .into_iter()
        .find(|p| p.exists());
    if existing.is_some() && !force {
        bail!("a sudsy config already exists here; not overwriting it (use --force to replace it)");
    }
    Ok(existing.unwrap_or_else(|| PathBuf::from("sudsy.yaml")))
}

/// The questions (or, with `--yes`, their defaults), then the files.
fn interactive(args: InitArgs) -> Result<()> {
    let path = config_path(args.force)?;
    let providers = providers();
    let dir = std::env::current_dir()?;
    let mut world = World::load(&dir, &providers, true)?;
    for &tool in &args.from {
        if !world.tools.iter().any(|f| f.tool == tool) {
            world.tools.push(detect_tool(tool)?);
        }
    }
    let flags = ask::Flags {
        languages: (!args.languages.is_empty()).then(|| args.languages.clone()),
        presets: args.presets.clone(),
        from: args.from.clone(),
        agents: args.agents,
        vscode: args.vscode,
        ci: args.ci,
    };
    let mut prompter: Box<dyn Prompter> = if args.yes {
        Box::new(prompt::Defaults)
    } else if std::env::var("TERM").is_ok_and(|t| t == "dumb") {
        Box::new(prompt::Lines {
            input: std::io::stdin().lock(),
            out: std::io::stderr(),
        })
    } else {
        Box::new(prompt::Terminal::new())
    };
    let answers = ask::ask(&world, &flags, &dir, prompter.as_mut())?;
    let header = header(&args.from);
    let plan = plan::plan(&world, &answers, &dir, &path, &header)?;
    for note in &plan.notes {
        prompter.say(&format!("note: {note}"));
    }
    prompter.say("Files to write:");
    for f in &plan.files {
        let verb = if f.action == Action::Create {
            "create"
        } else {
            "update"
        };
        prompter.say(&format!("  {verb} {} ({})", shown(&f.path, &dir), f.note));
    }
    if let Some(config) = plan.files.first() {
        prompter.say(&format!(
            "\n{}",
            config.text.trim_end().replace('\n', "\n  | ")
        ));
    }
    if !args.yes && !prompter.confirm("Write these files", true)? {
        return Err(Cancelled.into());
    }
    write(&plan, &dir)?;
    let unformatted = settle(&plan, &dir);
    finish(&plan, args.write_editorconfig)?;
    eprintln!(
        "next: run {}`sudsy check`",
        if unformatted {
            "`sudsy format`, then "
        } else {
            ""
        }
    );
    Ok(())
}

/// Runs Sudsy on `paths` in `dir`; whether it exited with success.
fn sudsy(dir: &Path, args: &[&str], paths: &[&Path]) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    std::process::Command::new(exe)
        .args(args)
        .args(paths)
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Puts the files `init` created through Sudsy's own safe fixes and formatter, with the config
/// just written. Returns whether a file the user already had does not pass the formatter: those
/// are not reformatted.
fn settle(plan: &Plan, dir: &Path) -> bool {
    let of = |action: Action| -> Vec<&Path> {
        plan.files
            .iter()
            .filter(|f| f.action == action)
            .map(|f| f.path.as_path())
            .collect()
    };
    let (created, updated) = (of(Action::Create), of(Action::Update));
    if !created.is_empty() {
        sudsy(dir, &["fix"], &created);
    }
    !updated.is_empty() && !sudsy(dir, &["format", "--check"], &updated)
}

fn shown(path: &Path, dir: &Path) -> String {
    path.strip_prefix(dir)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn header(from: &[Tool]) -> String {
    let flags: String = from
        .iter()
        .map(|t| format!(" --from {}", tool_name(*t)))
        .collect();
    format!("Generated by `sudsy init{flags}`; review before use.")
}

fn tool_name(tool: Tool) -> String {
    tool.to_possible_value()
        .expect("named")
        .get_name()
        .to_string()
}

fn detect_tool(tool: Tool) -> Result<Found> {
    let (out, source) = migrate(tool)?;
    Ok(Found {
        tool,
        name: tool_name(tool),
        source,
        out,
    })
}

/// Creates and updates the files of the plan. A file that is new is never replaced.
fn write(plan: &Plan, dir: &Path) -> Result<()> {
    let mut done: Vec<String> = Vec::new();
    for f in &plan.files {
        let step = (|| -> Result<()> {
            match f.action {
                Action::Create => {
                    if let Some(parent) = f.path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&f.path)
                        .with_context(|| {
                            format!(
                                "{} appeared meanwhile; not overwriting it",
                                f.path.display()
                            )
                        })?;
                    std::io::Write::write_all(&mut file, f.text.as_bytes())?;
                }
                Action::Update => crate::atomic::write(&f.path, f.text.as_bytes())?,
            }
            Ok(())
        })();
        if let Err(e) = step {
            let written = if done.is_empty() {
                "nothing was written".to_string()
            } else {
                format!("written: {}", done.join(", "))
            };
            return Err(e.context(format!(
                "could not write {} ({written})",
                shown(&f.path, dir)
            )));
        }
        eprintln!("wrote {}", shown(&f.path, dir));
        done.push(shown(&f.path, dir));
    }
    Ok(())
}

/// What the migrations report, on stdout, and the `.editorconfig` entries of the C# tools.
fn finish(plan: &Plan, write_editorconfig: bool) -> Result<()> {
    for (source, report) in &plan.reports {
        if !source.is_empty() {
            print_report(source, report);
        }
        if write_editorconfig && !report.editorconfig.is_empty() {
            let added = editorconfig::append(&report.editorconfig)?;
            println!(
                "{}",
                if added {
                    "appended to .editorconfig"
                } else {
                    ".editorconfig already has the entries"
                }
            );
        }
    }
    Ok(())
}

/// The no-terminal path: the starter, or the migration of the tools named by `--from`. The files
/// are the ones the questions would write for the same answers.
fn init(args: InitArgs) -> Result<()> {
    let path = config_path(args.force)?;
    if args.from.is_empty() {
        let providers = providers();
        let presets: Vec<String> = providers
            .iter()
            .flat_map(|p| p.option_schema().presets())
            .map(|p| p.name.clone())
            .collect();
        let text = starter(args.presets, &presets)?;
        std::fs::write(&path, text)?;
        eprintln!("wrote {}", path.display());
        return Ok(());
    }
    let wants_editorconfig = |t: &Tool| matches!(t, Tool::Stylecop | Tool::Csharpier);
    if args.write_editorconfig && !args.from.iter().any(wants_editorconfig) {
        bail!("--write-editorconfig applies to --from stylecop and --from csharpier");
    }
    let providers = providers();
    let mut world = World::bare(&providers);
    for &tool in &args.from {
        world.tools.push(detect_tool(tool)?);
    }
    let mut answers = Answers {
        tools: (0..world.tools.len()).collect(),
        ..Answers::default()
    };
    ask::apply_presets(&world, &args.presets, &mut answers)?;
    ask::keep_tool_styles(&world, &mut answers);
    let dir = std::env::current_dir()?;
    let plan = plan::plan(&world, &answers, &dir, &path, &header(&args.from))?;
    write(&plan, &dir)?;
    finish(&plan, args.write_editorconfig)
}

fn starter(presets: Vec<String>, known: &[String]) -> Result<String> {
    let mut text = String::from("# https://github.com/bingsubandit/sudsy\n");
    text.push_str(if presets.is_empty() {
        "extends: []   # e.g. preset:python/ruff\n"
    } else {
        "extends:\n"
    });
    for preset in &presets {
        if !known.contains(preset) {
            bail!(
                "unknown preset `{preset}`; known presets: {}",
                known.join(", ")
            );
        }
        text.push_str(&format!("  - preset:{preset}\n"));
    }
    Ok(text)
}

/// Reads the tool's config and maps it. Returns what was read, for the report.
fn migrate(tool: Tool) -> Result<(Out, String)> {
    let found = |what: &str, names: &[&str]| {
        NotFound(format!(
            "no {what} config found here (looked for {})",
            names.join(", ")
        ))
    };
    match tool {
        Tool::Yamllint => {
            let path = sources::find(&yamllint::FILES)
                .ok_or_else(|| found("yamllint", &yamllint::FILES))?;
            let mut out = Out::new("yamllint");
            yamllint::migrate(&sources::parse(&path)?, &mut out)
                .with_context(|| path.display().to_string())?;
            Ok((out, path.display().to_string()))
        }
        Tool::Prettier => {
            let mut out = Out::new("prettier");
            let (config, source) = match sources::find(&prettier::FILES) {
                Some(path) => (sources::parse(&path)?, path.display().to_string()),
                None => match sources::package_json_key("prettier")? {
                    Some(v) => (v, "package.json".to_string()),
                    None => match sources::find(&prettier::UNSUPPORTED) {
                        Some(p) => bail!(
                            "{} is not JSON or YAML; only .prettierrc, .prettierrc.json/.yaml/.yml and package.json \"prettier\" are read",
                            p.display()
                        ),
                        None => return Err(found("Prettier", &prettier::FILES).into()),
                    },
                },
            };
            prettier::migrate(&config, &mut out).with_context(|| source.clone())?;
            out.ignore_file(".prettierignore")?;
            Ok((out, source))
        }
        Tool::Biome => {
            let path = sources::find(&biome::FILES).ok_or_else(|| found("Biome", &biome::FILES))?;
            let mut out = Out::new("biome");
            biome::migrate(&sources::parse(&path)?, &mut out)
                .with_context(|| path.display().to_string())?;
            Ok((out, path.display().to_string()))
        }
        Tool::Eslint => {
            let mut out = Out::new("eslint");
            let (config, source) = match sources::find(&eslint::FILES) {
                Some(path) => (sources::parse(&path)?, path.display().to_string()),
                None => match sources::package_json_key("eslintConfig")? {
                    Some(v) => (v, "package.json".to_string()),
                    None => match sources::find(&eslint::JS_CONFIGS) {
                        Some(p) => bail!(
                            "{} is JavaScript and cannot be read without running it; only JSON/YAML legacy configs (.eslintrc.json/.yaml/.yml, package.json \"eslintConfig\") are supported. Write the options by hand: docs/ai/parity/eslint.yaml, typescript-eslint.yaml and eslint-plugin-jsonc.yaml map each rule",
                            p.display()
                        ),
                        None => return Err(found("ESLint", &eslint::FILES).into()),
                    },
                },
            };
            if let Some(js) = sources::find(&eslint::JS_CONFIGS) {
                out.note(format!("{} is JavaScript and was not read", js.display()));
            }
            eslint::migrate(&config, &mut out).with_context(|| source.clone())?;
            out.ignore_file(eslint::IGNORE_FILE)?;
            Ok((out, source))
        }
        Tool::Stylelint => {
            let mut out = Out::new("stylelint");
            let (config, source) = match sources::find(&stylelint::FILES) {
                Some(path) => (sources::parse(&path)?, path.display().to_string()),
                None => match sources::package_json_key("stylelint")? {
                    Some(v) => (v, "package.json".to_string()),
                    None => match sources::find(&stylelint::JS_CONFIGS) {
                        Some(p) => bail!(
                            "{} is JavaScript and cannot be read without running it; only JSON/YAML configs (.stylelintrc, .stylelintrc.json/.yaml/.yml, package.json \"stylelint\") are supported. Write the rules by hand: docs/ai/parity/stylelint.yaml maps each rule",
                            p.display()
                        ),
                        None => return Err(found("stylelint", &stylelint::FILES).into()),
                    },
                },
            };
            if let Some(js) = sources::find(&stylelint::JS_CONFIGS) {
                out.note(format!("{} is JavaScript and was not read", js.display()));
            }
            stylelint::migrate(&config, &mut out).with_context(|| source.clone())?;
            out.ignore_file(stylelint::IGNORE_FILE)?;
            Ok((out, source))
        }
        Tool::Stylecop => {
            let file = sources::find(&stylecop::FILES);
            let settings = file.as_deref().map(sources::parse).transpose()?;
            if settings.is_none() && editorconfig::sa_severities()?.is_empty() {
                return Err(NotFound(format!(
                    "{}; and no dotnet_diagnostic.SA*.severity in .editorconfig",
                    found("StyleCop", &stylecop::FILES)
                ))
                .into());
            }
            let mut out = Out::new("stylecop");
            stylecop::migrate(settings.as_ref(), &mut out)?;
            let source = file
                .as_deref()
                .map_or(".editorconfig".into(), |p: &Path| p.display().to_string());
            Ok((out, source))
        }
        Tool::HtmlValidate => {
            let Some(path) = sources::find(&html::HTML_VALIDATE_FILES) else {
                return Err(match sources::find(&html::HTML_VALIDATE_JS) {
                    Some(p) => anyhow::anyhow!(
                        "{} is JavaScript and cannot be read without running it; only .htmlvalidate.json is supported. Write the rules by hand: docs/ai/parity/html-validate.yaml maps each rule",
                        p.display()
                    ),
                    None => found("html-validate", &html::HTML_VALIDATE_FILES).into(),
                });
            };
            let mut out = Out::new("html-validate");
            html::html_validate(&sources::parse(&path)?, &mut out)
                .with_context(|| path.display().to_string())?;
            Ok((out, path.display().to_string()))
        }
        Tool::Htmlhint => {
            let path = sources::find(&html::HTMLHINT_FILES)
                .ok_or_else(|| found("HTMLHint", &html::HTMLHINT_FILES))?;
            let mut out = Out::new("htmlhint");
            html::htmlhint(&sources::parse(&path)?, &mut out)
                .with_context(|| path.display().to_string())?;
            Ok((out, path.display().to_string()))
        }
        Tool::HtmlEslint => {
            let mut out = Out::new("html-eslint");
            let (config, source) = match sources::find(&eslint::FILES) {
                Some(path) => (sources::parse(&path)?, path.display().to_string()),
                None => match sources::package_json_key("eslintConfig")? {
                    Some(v) => (v, "package.json".to_string()),
                    None => match sources::find(&eslint::JS_CONFIGS) {
                        Some(p) => bail!(
                            "{} is JavaScript and cannot be read without running it; only JSON/YAML legacy configs (.eslintrc.json/.yaml/.yml, package.json \"eslintConfig\") are supported. Write the rules by hand: docs/ai/parity/html-eslint.yaml maps each rule",
                            p.display()
                        ),
                        None => return Err(found("ESLint", &eslint::FILES).into()),
                    },
                },
            };
            html::html_eslint(&config, &mut out).with_context(|| source.clone())?;
            Ok((out, source))
        }
        Tool::Csharpier => {
            let path = sources::find(&csharpier::FILES)
                .ok_or_else(|| found("CSharpier", &csharpier::FILES))?;
            let mut out = Out::new("csharpier");
            csharpier::migrate(&sources::parse(&path)?, &mut out)
                .with_context(|| path.display().to_string())?;
            Ok((out, path.display().to_string()))
        }
    }
}

fn print_report(source: &str, report: &Report) {
    println!("migrated {source}");
    for preset in &report.extends {
        println!("extends: preset:{preset}");
    }
    for item in &report.not_migrated {
        println!("not migrated: {item}");
    }
    for note in &report.notes {
        println!("note: {note}");
    }
    if !report.editorconfig.is_empty() {
        println!("\nAdd to .editorconfig (or run again with --write-editorconfig):\n");
        println!("{}", editorconfig::block(&report.editorconfig));
        println!(
            "note: set EnforceCodeStyleInBuild to true (Directory.Build.props) so the SDK reports the other IDE* style rules during `dotnet build`"
        );
    }
}
