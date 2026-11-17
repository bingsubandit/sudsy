//! What an import declaration binds: the form it is written in, what it says in a message and
//! what the bound name stands for.

use crate::generated::kind as k;
use crate::semantic::Cx;
use crate::text::code_text;
use core_semantics::scopes::{Decl, Flags};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    /// `import a`, `import a as b`.
    Import,
    /// `import a.b`: binds `a`.
    Submodule,
    /// `from m import a`.
    From,
    /// `from __future__ import a`.
    Future,
}

pub struct Import {
    pub form: Form,
    /// What a message says (`a.b`, `.m.a`).
    pub label: String,
    /// What the bound name stands for (`a` for `import a.b`).
    pub target: String,
    /// `import a as a`, `from m import a as a`: a re-export.
    pub explicit: bool,
}

/// The import an `Alias` declaration makes; `None` for a declaration of another kind.
pub fn describe(cx: &Cx, d: &Decl) -> Option<Import> {
    if !d.flags.contains(Flags::IMPORT) {
        return None;
    }
    let alias = d.node(&cx.root);
    let name = code_text(&cx.slot(&alias, "name")?);
    let asname = cx.slot(&alias, "asname").map(|a| code_text(&a));
    let stmt = alias.parent()?;
    if stmt.kind() == k::STMT_IMPORT {
        return Some(match asname {
            Some(a) => Import {
                form: Form::Import,
                explicit: a == name,
                label: name.clone(),
                target: name,
            },
            None => {
                let first = name.split('.').next().unwrap_or(&name).to_owned();
                let form = if first.len() < name.len() {
                    Form::Submodule
                } else {
                    Form::Import
                };
                Import {
                    form,
                    label: name,
                    target: first,
                    explicit: false,
                }
            }
        });
    }
    let module = cx
        .slot(&stmt, "module")
        .map(|m| code_text(&m))
        .unwrap_or_default();
    let level: usize = stmt
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .map(|t| match t.kind() {
            k::DOT => 1,
            k::ELLIPSIS => 3,
            _ => 0,
        })
        .sum();
    let dots = ".".repeat(level);
    let full = if module.is_empty() {
        format!("{dots}{name}")
    } else {
        format!("{dots}{module}.{name}")
    };
    Some(Import {
        form: if level == 0 && module == "__future__" {
            Form::Future
        } else {
            Form::From
        },
        explicit: asname.is_some_and(|a| a == name),
        label: full.clone(),
        target: full,
    })
}

/// What the name a declaration binds stands for, when it is an import.
pub fn target(cx: &Cx, d: &Decl) -> Option<String> {
    describe(cx, d).map(|i| i.target)
}
