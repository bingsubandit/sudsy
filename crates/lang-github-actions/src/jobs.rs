//! Rules on the `jobs` of a workflow: ids, `needs`, `runs-on` and `permissions`.
//!
//! concept: new invalid-job-id, new duplicate-ids, new unknown-needs, new cyclic-needs, new invalid-runs-on, new missing-permissions
//! what: invalid-job-id: A job id must start with a letter or `_` and contain only letters, digits, `-` and `_`
//! what: duplicate-ids: Job ids that differ only in case (ids are case-insensitive; exact repeats are `no-duplicate-keys`), and step ids repeated within a job
//! what: unknown-needs: `needs` names a job that the workflow does not define
//! what: cyclic-needs: The `needs` of the jobs form a cycle, so no job of it can start
//! what: invalid-runs-on: A `runs-on` label that is not one of the known runner labels
//! what: missing-permissions: A job without `permissions`, in a workflow without top-level `permissions`: it gets the repository's default token permissions, often write access

use crate::view::{Entry, entries, get, items, root_mapping, scalar, scalar_list};
use core_rules::rule;
use core_rules::unknown_reference::{self, Item, Scope};
use core_rules::{Check, FileContext, Meta, Opt, OptionDecl, Re, RuleContext, meta_regex};
use core_syntax::{SyntaxNode, TextRange};
use std::collections::HashMap;
use std::sync::LazyLock;

/// The `jobs` mapping of a workflow; empty for any other file (an `action.yml`).
fn jobs(file: &FileContext<'_>) -> Vec<Entry> {
    root_mapping(file.root)
        .and_then(|root| get(&root, file.text, "jobs"))
        .and_then(|e| e.value)
        .map(|v| entries(&v, file.text))
        .unwrap_or_default()
}

rule! {
    pub InvalidJobId {
        id: "invalid-job-id",
        category: Correctness,
        default: Error,
        description: "Reports a job id that does not start with a letter or `_`, or that has a character \
                  other than letters, digits, `-` and `_`.",
        why: "GitHub rejects the workflow with a message about the id, and in expressions the id \
                  is used as a name (`needs.<id>`), where other characters do not work.\n\
                  To fix, rename the job.",
        example: "jobs:\n  build app:   # reported\n    runs-on: ubuntu-latest",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for job in jobs(file) {
            let mut chars = job.key.chars();
            let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if !valid {
                ctx.report(job.key_range, format!("invalid job id `{}`", job.key));
            }
        }
    }
}

rule! {
    pub DuplicateIds {
        id: "duplicate-ids",
        category: Correctness,
        default: Error,
        description: "Reports job ids that differ only in case (job ids are not case sensitive; exact \
                  repeats are a different rule, `no-duplicate-keys`), and step ids repeated within a \
                  job.",
        why: "Ids are compared without case, so a reference such as `needs.build` cannot tell two such jobs apart, and `steps.<id>.outputs` cannot tell two steps with one id apart.\n\
                  To fix, give each job and step its own id.",
        example: "jobs:\n  Build: {}\n  build: {}   # reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut seen: HashMap<String, String> = HashMap::new();
        for job in jobs(file) {
            let lower = job.key.to_lowercase();
            match seen.get(&lower) {
                Some(first) if *first != job.key => {
                    let message = format!("job id `{}` repeats `{first}` (ids are case-insensitive)", job.key);
                    ctx.report(job.key_range, message);
                }
                Some(_) => {}
                None => {
                    seen.insert(lower, job.key.clone());
                }
            }
            let steps = job
                .value
                .and_then(|v| get(&v, file.text, "steps"))
                .and_then(|s| s.value);
            let mut ids: HashMap<String, ()> = HashMap::new();
            for step in steps.iter().flat_map(crate::view::items) {
                let Some((id, range)) = get(&step, file.text, "id")
                    .and_then(|e| e.value)
                    .and_then(|v| scalar(&v, file.text))
                else {
                    continue;
                };
                if ids.insert(id.to_lowercase(), ()).is_some() {
                    ctx.report(range, format!("step id `{id}` repeats within job `{}`", job.key));
                }
            }
        }
    }
}

/// `(job, needs entries)` of every job.
fn needs(file: &FileContext<'_>) -> Vec<(String, Vec<(String, TextRange)>)> {
    jobs(file)
        .into_iter()
        .map(|job| {
            let list = job
                .value
                .and_then(|v| get(&v, file.text, "needs"))
                .and_then(|e| e.value)
                .map(|v| scalar_list(&v, file.text))
                .unwrap_or_default();
            (job.key, list)
        })
        .collect()
}

rule! {
    pub UnknownNeeds {
        id: "unknown-needs",
        category: Correctness,
        default: Error,
        description: "Reports a `needs` entry that names a job that the workflow does not define.",
        why: "GitHub rejects the workflow. The usual cause is a job that was renamed or removed \
                  while another still waits for it.\n\
                  To fix, correct the name, or remove it from `needs`.",
        example: "jobs:\n  test:\n    needs: build   # reported\n    runs-on: ubuntu-latest",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let all = needs(file);
        let uses = all.iter().enumerate().flat_map(|(at, (_, l))| l.iter().map(move |(name, _)| Item::Use { name: name.clone(), tag: at }));
        let items: Vec<Item> = all.iter().map(|(job, _)| Item::Declare(job.clone())).chain(uses).collect();
        let report = |at: usize, name: &str| (jobs(file)[at].key_range, format!("job `{}` needs unknown job `{name}`", all[at].0));
        let scope = Scope { earlier_only: false, ignore_case: true };
        unknown_reference::check(&items, &scope, |_, _| false, report, ctx);
    }
}

rule! {
    pub CyclicNeeds {
        id: "cyclic-needs",
        category: Correctness,
        default: Error,
        description: "Reports jobs whose `needs` form a cycle, such as `a` needing `b` and `b` needing \
                  `a`.",
        why: "A job starts when the jobs it needs have finished. In a cycle each job waits for \
                  another, so none of them can start, and GitHub rejects the workflow.\n\
                  To fix, remove one of the `needs` that closes the cycle.",
        example: "jobs:\n  a:\n    needs: b   # reported\n  b:\n    needs: a   # reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let all = needs(file);
        let keys = jobs(file);
        let index = |name: &str| all.iter().position(|(j, _)| j.eq_ignore_ascii_case(name));
        // Whether `to` is reached from `from` through one or more `needs`.
        let reaches = |from: usize, to: usize| {
            let mut stack: Vec<usize> = all[from].1.iter().filter_map(|(n, _)| index(n)).collect();
            let mut seen = vec![false; all.len()];
            while let Some(at) = stack.pop() {
                if at == to {
                    return true;
                }
                if !std::mem::replace(&mut seen[at], true) {
                    stack.extend(all[at].1.iter().filter_map(|(n, _)| index(n)));
                }
            }
            false
        };
        // One report for each cycle, at its first job.
        let mut done = vec![false; all.len()];
        for (start, (job, _)) in all.iter().enumerate() {
            if done[start] || !reaches(start, start) {
                continue;
            }
            for (other, d) in done.iter_mut().enumerate() {
                *d |= other == start || (reaches(start, other) && reaches(other, start));
            }
            ctx.report(keys[start].key_range, format!("job `{job}` is part of a cycle of `needs`"));
        }
    }
}

static MATRIX_REF: LazyLock<Meta> =
    LazyLock::new(|| meta_regex(r"^\$\{\{\s*matrix\.([\w-]+)\s*\}\}$").expect("a valid pattern"));

fn matrix_ref(label: &str) -> Option<&str> {
    let mut caps = MATRIX_REF.create_captures();
    MATRIX_REF.captures(label, &mut caps);
    caps.get_group(1).map(|s| &label[s.range()])
}

/// The scalars of the job's `strategy.matrix.<name>` and of that key in its `include` entries.
fn matrix_values(job: &SyntaxNode, text: &str, name: &str) -> Vec<(String, TextRange)> {
    let Some(matrix) = get(job, text, "strategy")
        .and_then(|s| s.value)
        .and_then(|s| get(&s, text, "matrix"))
        .and_then(|m| m.value)
    else {
        return Vec::new();
    };
    let mut out: Vec<_> = get(&matrix, text, name)
        .and_then(|e| e.value)
        .map(|v| scalar_list(&v, text))
        .unwrap_or_default();
    let include = get(&matrix, text, "include").and_then(|e| e.value);
    for entry in include.iter().flat_map(items) {
        out.extend(
            get(&entry, text, name)
                .and_then(|e| e.value)
                .and_then(|v| scalar(&v, text)),
        );
    }
    out
}

/// The GitHub-hosted images and the labels every self-hosted runner has.
const HOSTED: &[&str] = &[
    r"ubuntu-slim",
    r"ubuntu-latest(-(4|8|16)-cores)?",
    r"ubuntu-(24|22)\.04(-arm)?",
    r"windows-latest(-8-cores)?",
    r"windows-2025(-vs2026)?",
    r"windows-2022",
    r"windows-11-arm",
    r"macos-latest(-(xlarge|large))?",
    r"macos-(26|15)(-(intel|xlarge|large))?",
    r"macos-14(-(xlarge|large))?",
    "self-hosted",
    "linux",
    "macos",
    "windows",
    "x64",
    "arm",
    "arm64",
];

rule! {
    pub InvalidRunsOn {
        id: "invalid-runs-on",
        category: Suspicious,
        default: Warn,
        description: "Reports a `runs-on` label that is not one of the known runner labels. A job that \
                  also has `self-hosted` is not checked, `${{ matrix.name }}` stands for the values of \
                  the matrix, and other expressions are skipped.",
        why: "A label that no runner has leaves the job queued with no runner to take it. A \
                  misspelling such as `ubuntu-lates` is the usual cause. The option `labels` lists the \
                  known labels.\n\
                  To fix, use a label of a hosted runner, or add `self-hosted` for your own.",
        example: "jobs:\n  build:\n    runs-on: ubuntu-lates   # reported",
        options: [OptionDecl {
            name: "labels",
            default: Opt::List(HOSTED),
            check: Check::Regex,
            description: "Regular expressions; a label is valid when one matches all of it, ignoring case.",
        }],
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let known: Vec<_> = ctx
            .options
            .list("labels")
            .iter()
            .filter_map(|p| Re::lazy(&format!("^(?i:(?:{p}))$")).ok())
            .collect();
        for job in jobs(file) {
            let Some(job_node) = job.value else { continue };
            let Some(runs_on) = get(&job_node, file.text, "runs-on") else {
                continue;
            };
            let Some(runs_on) = runs_on.value else {
                ctx.report(runs_on.key_range, core_rules::expr::unknown_name(&ctx.options.list("labels"), "label", ""));
                continue;
            };
            // `runs-on: { group: g, labels: [a] }` names labels too.
            let labels = match get(&runs_on, file.text, "labels").and_then(|e| e.value) {
                Some(v) => scalar_list(&v, file.text),
                None => scalar_list(&runs_on, file.text),
            };
            // `${{ matrix.os }}` stands for the values the matrix gives `os`.
            let labels: Vec<_> = labels
                .into_iter()
                .flat_map(|(label, range)| match matrix_ref(&label) {
                    Some(name) => matrix_values(&job_node, file.text, name),
                    None => vec![(label, range)],
                })
                .collect();
            if labels.iter().any(|(l, _)| l.eq_ignore_ascii_case("self-hosted")) {
                continue;
            }
            for (label, range) in labels {
                if !label.contains("${{") && !known.iter().any(|k| k.is_match(&label)) {
                    ctx.report(range, core_rules::expr::unknown_name(&ctx.options.list("labels"), "label", &label));
                }
            }
        }
    }
}

rule! {
    pub MissingPermissions {
        id: "missing-permissions",
        category: Suspicious,
        default: Off,
        description: "Reports a job with no `permissions`, in a workflow with no top-level `permissions`.",
        why: "Without `permissions`, the `GITHUB_TOKEN` gets the default of the repository or \
                  organization, which for older repositories is read and write on most scopes. A \
                  compromised step or action then has more access than the job needs.\n\
                  To fix, set `permissions` with only the scopes the job uses, for example `contents: \
                  read`.",
        example: "jobs:\n  build:   # reported\n    runs-on: ubuntu-latest",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(root) = root_mapping(file.root) else { return };
        if get(&root, file.text, "permissions").is_some() {
            return;
        }
        for job in jobs(file) {
            if !job.value.is_some_and(|v| get(&v, file.text, "permissions").is_some()) {
                ctx.report(job.key_range, format!("job `{}` has no `permissions`", job.key));
            }
        }
    }
}
