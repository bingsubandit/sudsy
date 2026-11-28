//! The one place that turns the answers of `sudsy init` (from questions or flags) into files.

use super::out::{Conflict, Out, Report};
use super::world::World;
use super::{agents, editor, hooks};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Default, Debug)]
pub struct Answers {
    /// Languages found and left out of the config.
    pub deselected: BTreeSet<String>,
    /// The style preset of a language; no entry: Sudsy's defaults.
    pub styles: BTreeMap<String, String>,
    /// Presets that only set rule levels.
    pub lint_presets: Vec<String>,
    /// Indexes into `World::tools`.
    pub tools: Vec<usize>,
    /// Paths of conflicts settled for the later tool.
    pub take_later: BTreeSet<String>,
    /// Instruction files that get the Sudsy block.
    pub agents: Vec<&'static str>,
    pub vscode: bool,
    pub format_on_save: bool,
    pub ci: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Action {
    Create,
    /// An existing file whose content changes; `sudsy.yaml` with `--force` is one.
    Update,
}

pub struct File {
    pub path: PathBuf,
    pub text: String,
    pub action: Action,
    /// What the file is for, for the summary.
    pub note: String,
}

#[derive(Default)]
pub struct Plan {
    pub files: Vec<File>,
    /// Migration reports, by tool.
    pub reports: Vec<(String, Report)>,
    /// Said before the files are written: what was left out and why.
    pub notes: Vec<String>,
}

/// The config tree the answers describe, and the conflicts between the tools it merges.
pub fn config(world: &World, a: &Answers) -> (Out, Vec<Conflict>, Vec<usize>, Vec<String>) {
    let mut out = Out::new("");
    for preset in a.styles.values().chain(&a.lint_presets) {
        out.extend(preset);
    }
    let mut conflicts = Vec::new();
    let mut used = Vec::new();
    let mut dropped = Vec::new();
    for &i in &a.tools {
        let found = &world.tools[i];
        let mut tool = found.out.clone();
        // Where the user picked another style the tool's settings for that language stay out.
        let covered = world.covered(&found.out, &a.deselected);
        tool.restrict(|l| {
            covered.iter().any(|c| c == l)
                && a.styles.get(l).map(String::as_str) == world.own_style(&found.out, l)
        });
        for lang in &covered {
            if tool.languages().contains(lang) {
                continue;
            }
            let style = a
                .styles
                .get(lang)
                .map_or("Sudsy's defaults", String::as_str);
            let settings = found.out.settings_of(lang);
            if !settings.is_empty() {
                dropped.push(format!(
                    "{lang} {} of {}: {lang} follows {style}",
                    settings.join(" and "),
                    found.name
                ));
            }
        }
        if !tool.is_empty() {
            used.push(i);
            conflicts.extend(out.merge(tool, &|path| a.take_later.contains(path)));
        }
    }
    (out, conflicts, used, dropped)
}

/// The globs that leave the deselected languages out, and the languages that could not be left
/// out because a glob would also catch a file of a language that stays.
fn exclusions(world: &World, a: &Answers) -> (Vec<String>, Vec<String>) {
    let (mut globs, mut stuck) = (Vec::new(), Vec::new());
    for (i, lang) in world.langs.iter().enumerate() {
        if !a.deselected.contains(&lang.name) {
            continue;
        }
        let mut kept_all = !lang.globs.is_empty();
        for glob in &lang.globs {
            let Ok(g) = globset::Glob::new(glob) else {
                kept_all = false;
                continue;
            };
            let m = g.compile_matcher();
            let catches_other = world.files.iter().any(|(path, at)| {
                *at != i && !a.deselected.contains(&world.langs[*at].name) && m.is_match(path)
            });
            if catches_other {
                kept_all = false;
            } else {
                globs.push(glob.clone());
            }
        }
        if !kept_all {
            stuck.push(lang.name.clone());
        }
    }
    (globs, stuck)
}

/// A file that must not exist yet, or one with new content.
fn file(dir: &Path, rel: &str, text: String, note: &str) -> File {
    let path = dir.join(rel);
    let action = if path.exists() {
        Action::Update
    } else {
        Action::Create
    };
    File {
        path,
        text,
        action,
        note: note.to_string(),
    }
}

pub fn plan(
    world: &World,
    a: &Answers,
    dir: &Path,
    sudsy_path: &Path,
    header: &str,
) -> Result<Plan> {
    let mut plan = Plan::default();
    let (mut out, _, used, dropped) = config(world, a);
    let (globs, stuck) = exclusions(world, a);
    for glob in &globs {
        out.exclude(glob);
    }
    if !stuck.is_empty() {
        plan.notes.push(format!(
            "{} still processed in part: a pattern that selects them also selects files of a language you kept",
            stuck.join(", ")
        ));
    }
    let known: Vec<(&str, &'static str)> = world
        .presets
        .iter()
        .map(|p| (p.name.as_str(), p.text))
        .collect();
    let (text, mut report) = out.render(header, &known);
    report.not_migrated.extend(dropped);
    let sources: Vec<&str> = used
        .iter()
        .map(|&i| world.tools[i].source.as_str())
        .collect();
    plan.reports.push((sources.join(", "), report));
    plan.files.push(File {
        path: sudsy_path.to_path_buf(),
        text,
        action: if sudsy_path.exists() {
            Action::Update
        } else {
            Action::Create
        },
        note: "the config".into(),
    });
    for name in &a.agents {
        let old = std::fs::read_to_string(dir.join(name)).unwrap_or_default();
        match agents::with_block(&old) {
            Some(text) => plan
                .files
                .push(file(dir, name, text, "how agents run Sudsy")),
            None => plan
                .notes
                .push(format!("{name} already has the Sudsy block")),
        }
    }
    if !a.agents.is_empty() {
        hooks::add(dir, &mut plan);
    }
    if a.vscode {
        let ids: Vec<&str> = world
            .langs
            .iter()
            .filter(|l| !a.deselected.contains(&l.name))
            .flat_map(|l| &l.vscode)
            .map(String::as_str)
            .collect();
        editor::add(dir, &ids, a.format_on_save, &mut plan);
    }
    if a.ci {
        editor::ci(dir, &mut plan);
    }
    Ok(plan)
}
