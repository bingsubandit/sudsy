//! `sudsy lint [paths...]`, and what `check` and `fix` share with it: gathering the diagnostics of
//! the files and printing them.

use crate::new::{Base, Entry, keep_new};
use crate::report::{self, Catalog, FileDiagnostics, OutputFormat};
use crate::session::Session;
use anyhow::{Result, anyhow};
use clap::{Args, ValueEnum};
use core_config::Depth;
use core_incremental::{
    File, Siblings, Stage, directory_scope, lint, module_diagnostics, project_diagnostics,
};
use core_rules::{SYNTAX_ERROR, Severity};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Analysis {
    Fast,
    Standard,
    Deep,
}

impl From<Analysis> for Depth {
    fn from(a: Analysis) -> Depth {
        match a {
            Analysis::Fast => Depth::Fast,
            Analysis::Standard => Depth::Standard,
            Analysis::Deep => Depth::Deep,
        }
    }
}

/// The options of every command that reports diagnostics.
#[derive(Args)]
pub struct ReportArgs {
    /// Files or directories; the current directory when omitted.
    pub paths: Vec<PathBuf>,
    /// How diagnostics are printed.
    #[arg(long, value_enum, default_value_t)]
    pub format: OutputFormat,
    /// Analysis depth; overrides `analysis` in sudsy.yaml.
    #[arg(long, value_enum)]
    pub analysis: Option<Analysis>,
    /// Report only the findings that are new against the git revision REV (`HEAD` when no value
    /// is given): the file's text there, linted with the current config. Exit code and summary
    /// count only those.
    #[arg(
        long,
        value_name = "REV",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "HEAD"
    )]
    pub new: Option<String>,
}

/// The options of `lint` and `check`.
#[derive(Args)]
pub struct LintArgs {
    #[command(flatten)]
    pub report: ReportArgs,
    /// Report only these rules (comma-separated or repeated); the human report is then never
    /// collapsed.
    #[arg(long, value_delimiter = ',')]
    pub rule: Vec<String>,
}

impl LintArgs {
    /// Keeps the findings of the rules `--rule` names, and with `--new` the new ones; prints the
    /// report.
    pub fn print(
        &self,
        session: &Session,
        base: Option<&Base>,
        loaded: &[Result<File>],
        files: &mut [FileDiagnostics],
    ) -> Result<()> {
        if !self.rule.is_empty() {
            for file in files.iter_mut() {
                file.diagnostics.retain(|d| self.rule.contains(&d.rule));
            }
        }
        if let Some(base) = base {
            let handles: HashMap<String, (&PathBuf, File)> = session
                .paths
                .iter()
                .zip(loaded)
                .filter_map(|(path, file)| {
                    Some((session.project.show(path), (path, *file.as_ref().ok()?)))
                })
                .collect();
            let entries = files
                .iter_mut()
                .filter_map(|report| {
                    let (path, file) = *handles.get(&report.path)?;
                    Some(Entry { path, file, report })
                })
                .collect();
            keep_new(&session.db, base, entries)?;
        }
        let report = Report {
            format: self.report.format,
            collapse: self.rule.is_empty(),
            new: base.is_some(),
        };
        print(session, &report, files);
        Ok(())
    }
}

impl ReportArgs {
    pub fn open(&self) -> Result<Session> {
        Session::open(&self.paths, self.analysis.map(Depth::from))
    }

    /// The revision of `--new`, checked before any file is read or written.
    pub fn base(&self) -> Result<Option<Base>> {
        self.new.as_deref().map(Base::open).transpose()
    }
}

/// How a report is printed.
pub struct Report {
    pub format: OutputFormat,
    /// As in [`report::render`].
    pub collapse: bool,
    /// Only new findings are left (`--new`): the summary says so, and is left out when there are
    /// none.
    pub new: bool,
}

/// The diagnostics of all files, and how many files could not be linted.
pub struct Collected {
    pub files: Vec<FileDiagnostics>,
    pub failed: usize,
}

/// Reads the memoized `lint` results of `loaded` (in `session.paths` order) and the project phase.
/// A file that could not be read or linted is reported on stderr and counted.
pub fn collect(session: &Session, loaded: &[anyhow::Result<File>]) -> Result<Collected> {
    let (db, project) = (&session.db, &session.project);
    let mut out = Collected {
        files: Vec::new(),
        failed: 0,
    };
    let mut ok_files = Vec::new();
    let mut warned = std::collections::HashSet::new();
    for (path, file) in session.paths.iter().zip(loaded) {
        let file = match file {
            Ok(file) => *file,
            Err(e) => {
                out.failed += 1;
                eprintln!("error: {}: {e:#}", project.show(path));
                continue;
            }
        };
        match lint(db, file) {
            Ok(linted) => {
                if let Some(why) = &linted.semantics_unavailable
                    && warned.insert(why.clone())
                {
                    eprintln!("warning: semantic rules are skipped: {why}");
                }
                ok_files.push(file);
                out.files.push(FileDiagnostics {
                    path: project.show(path),
                    text: file.text(db).clone(),
                    diagnostics: linted.diagnostics.clone(),
                });
            }
            Err(failure) if matches!(failure.stage, Stage::Options | Stage::Config) => {
                return Err(anyhow!("{}", failure.message));
            }
            Err(failure) => {
                out.failed += 1;
                report::file_error(&project.show(path), &failure.message);
            }
        }
    }
    let project_wide =
        project_diagnostics(db, &ok_files).map_err(|failure| anyhow!("{}", failure.message))?;
    for (path, diagnostic) in project_wide {
        let shown = project.show(&path);
        if let Some(file) = out.files.iter_mut().find(|f| f.path == shown) {
            file.diagnostics.push(diagnostic);
        }
    }
    add_directory_findings(session, &ok_files, &mut out)?;
    Ok(out)
}

/// The findings of the directory rules (a module's files judged together) on the files of the run.
/// The siblings of a file that the run does not name (or that the config ignores) give facts and
/// get no findings.
fn add_directory_findings(session: &Session, ok_files: &[File], out: &mut Collected) -> Result<()> {
    let (db, project) = (&session.db, &session.project);
    let mut dirs: std::collections::BTreeMap<PathBuf, (core_rules::DirectoryScope, Vec<File>)> =
        Default::default();
    for &file in ok_files {
        if let Some(scope) = directory_scope(db, file)
            && let Some(dir) = file.path(db).parent()
        {
            dirs.entry(dir.to_path_buf())
                .or_insert((scope, Vec::new()))
                .1
                .push(file);
        }
    }
    let siblings = Siblings::default();
    for (dir, (scope, own)) in dirs {
        let unit = siblings.unit(db, &dir, &scope, &own);
        let found = module_diagnostics(db, unit)
            .as_ref()
            .map_err(|failure| anyhow!("{}", failure.message))?;
        for (path, diagnostic) in found {
            let shown = project.show(path);
            if let Some(file) = out.files.iter_mut().find(|f| f.path == shown) {
                file.diagnostics.push(diagnostic.clone());
                file.diagnostics.sort_by(|a, b| {
                    (a.range.start(), a.range.end(), &a.rule).cmp(&(
                        b.range.start(),
                        b.range.end(),
                        &b.rule,
                    ))
                });
            }
        }
    }
    Ok(())
}

/// The rule table SARIF carries: every rule, plus the two ids that are not rules.
pub fn catalog(session: &Session) -> Catalog {
    use core_incremental::Db;
    let mut rules: Vec<(String, String)> =
        languages::rules(session.db.providers(), &session.project.extensions)
            .iter()
            .map(|(_, r)| (r.meta().id.to_string(), r.meta().summary()))
            .collect();
    rules.dedup();
    rules.push((SYNTAX_ERROR.into(), "The file does not parse.".into()));
    rules.push((
        core_rules::FORMAT.into(),
        "The file is not formatted; run `sudsy format`.".into(),
    ));
    rules
}

/// Prints the report on stdout and a one-line summary on stderr.
pub fn print(session: &Session, report: &Report, files: &[FileDiagnostics]) {
    print!(
        "{}",
        report::render(report.format, files, &catalog(session), report.collapse)
    );
    let (errors, warnings) = report::count(files);
    if !report.new {
        eprintln!(
            "{} file(s) checked, {errors} error(s), {warnings} warning(s)",
            files.len()
        );
    } else if errors + warnings > 0 {
        eprintln!(
            "{} file(s) checked, {errors} new error(s), {warnings} new warning(s)",
            files.len()
        );
    }
}

pub fn has_errors(files: &[FileDiagnostics]) -> bool {
    files
        .iter()
        .flat_map(|f| &f.diagnostics)
        .any(|d| d.severity == Severity::Error)
}

/// Exit code of the commands that report: 2 when a file could not be processed, else 1 on errors.
pub fn exit_code(collected: &Collected) -> ExitCode {
    if collected.failed > 0 {
        ExitCode::from(2)
    } else if has_errors(&collected.files) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

pub fn run(args: LintArgs) -> Result<ExitCode> {
    let base = args.report.base()?;
    let mut session = args.report.open()?;
    let loaded = session.load_linted(|db, file| {
        lint(db, file);
    });
    let mut collected = collect(&session, &loaded)?;
    args.print(&session, base.as_ref(), &loaded, &mut collected.files)?;
    Ok(exit_code(&collected))
}
