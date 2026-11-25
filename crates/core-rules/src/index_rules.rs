//! Rules that read the symbol index: what a file's names stand for in the other files of its
//! project. The index sits behind [`IndexView`] (core-incremental implements it over its queries);
//! a rule reports only on a proof (`Known` or `Absent`), never on `Unknown`, and a run without a
//! rule that reads the index builds none. Families (rule files name them with `family:`):
//! - `import-used`: an import none of whose declared names the file uses. Params: `message` with
//!   `{path}`. A public import re-exports and is left alone.
//! - `name-resolves`: a name the file writes in a reference position that no file of the project
//!   or its dependencies declares. Params: `message` with `{name}`. Reports only on `Absent`.
//! - `member-ref-unique`: a type that two methods name (as request or response, parameter or
//!   return type) is named by one method only. Params: `message` with `{type}`. The report is on
//!   each method that names a type some other method names too.
//!
//! concept: index-rules
//! what: A rule that reports from what the symbol index proves about the names a file writes: an import nothing uses, a type named by two methods.
//! tools: buf IMPORT_USED, buf RPC_REQUEST_RESPONSE_UNIQUE
//! rules: import-used, rpc-request-response-unique, class-not-found
//! reads: rule:import-used, rule:rpc-request-response-unique, rule:class-not-found

use crate::{ActiveRule, Diagnostic, Family, FileContext, Params, Rule, RuleContext, RuleMeta};
use core_semantics::symbols::{Answer, FileSites, Name};
use std::path::PathBuf;

/// A method somewhere in the project that names a type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Referrer {
    pub path: PathBuf,
    pub owner: Box<str>,
    pub method: Box<str>,
}

/// What a rule asks of the index about one file.
pub trait IndexView {
    /// What the file writes.
    fn sites(&self) -> &FileSites;

    /// Whether the file the import `path` names (with what it imports publicly) declares one of
    /// `names`; `Unknown` while the file or anything it imports is not read.
    fn import_declares(&self, path: &str, names: &[Name]) -> Answer<bool>;

    /// The qualified name a written name stands for.
    fn resolve(&self, name: &Name) -> Answer<Box<str>>;

    /// Every method of the project that names the qualified type `target`, one entry for each
    /// time it names it.
    fn referrers(&self, target: &str) -> Vec<Referrer>;
}

/// Runs the index rules among `active` on a file and returns what they report.
pub fn run_index_rules(
    file: &FileContext<'_>,
    active: &[ActiveRule<'_>],
    index: &dyn IndexView,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for a in active.iter().filter(|a| a.rule.reads_index()) {
        let mut ctx = RuleContext::new(file, &a.options, a.rule.meta(), a.severity, &mut out);
        a.rule.check_index(index, &mut ctx);
    }
    out
}

pub const IMPORT_USED: Family = Family {
    name: "import-used",
    build: build_import_used,
};

pub const NAME_RESOLVES: Family = Family {
    name: "name-resolves",
    build: build_name_resolves,
};

pub const MEMBER_REF_UNIQUE: Family = Family {
    name: "member-ref-unique",
    build: build_member_ref_unique,
};

struct ImportUsed {
    meta: &'static RuleMeta,
    message: String,
}

fn build_import_used(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let message = p.text("message")?;
    p.done()?;
    Ok(Box::new(ImportUsed { meta, message }))
}

impl Rule for ImportUsed {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn reads_index(&self) -> bool {
        true
    }

    fn check_index(&self, index: &dyn IndexView, ctx: &mut RuleContext<'_>) {
        let sites = index.sites();
        let uses: Vec<Name> = sites.uses.iter().map(|u| u.name.clone()).collect();
        for import in sites.imports.iter().filter(|i| !i.public) {
            if let Answer::Known(false) = index.import_declares(&import.path, &uses) {
                ctx.report(import.range, self.message.replace("{path}", &import.path));
            }
        }
    }
}

struct MemberRefUnique {
    meta: &'static RuleMeta,
    message: String,
}

fn build_member_ref_unique(
    meta: &'static RuleMeta,
    mut p: Params,
) -> Result<Box<dyn Rule>, String> {
    let message = p.text("message")?;
    p.done()?;
    Ok(Box::new(MemberRefUnique { meta, message }))
}

impl Rule for MemberRefUnique {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn reads_index(&self) -> bool {
        true
    }

    fn check_index(&self, index: &dyn IndexView, ctx: &mut RuleContext<'_>) {
        for method in &index.sites().methods {
            let shared = method.refs.iter().find_map(|name| {
                let Answer::Known(target) = index.resolve(name) else {
                    return None;
                };
                (index.referrers(&target).len() > 1).then_some(target)
            });
            if let Some(target) = shared {
                ctx.report(method.range, self.message.replace("{type}", &target));
            }
        }
    }
}

struct NameResolves {
    meta: &'static RuleMeta,
    message: String,
}

fn build_name_resolves(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let message = p.text("message")?;
    p.done()?;
    Ok(Box::new(NameResolves { meta, message }))
}

impl Rule for NameResolves {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn reads_index(&self) -> bool {
        true
    }

    fn check_index(&self, index: &dyn IndexView, ctx: &mut RuleContext<'_>) {
        for site in &index.sites().uses {
            if let Answer::Absent = index.resolve(&site.name) {
                ctx.report(site.range, self.message.replace("{name}", &site.written));
            }
        }
    }
}
