//! Checks on the commands of shell text embedded in a host (`RUN`, recipe lines, run steps), read
//! from the Shell tree of the region. A check selects commands by `program` and `args` and breaks
//! when one of them fails it; with no check param, the selected command itself breaks. Params:
//! - `program`: regex the whole command name matches; `args`: words of which one must be among the
//!   command's arguments (the subcommand, `install`). Both select the commands.
//! - `needs`: flags, one of which the command must have (`-y --yes -q*2`: `q` twice).
//! - `pinned` / `unpinned`: every package (the arguments after the subcommand that are not flags)
//!   must match `pinned` and must not match `unpinned`; `skip-flag-args`: flags whose value is
//!   not a package.
//! - `unless`: words (flags with a dash) that exempt the command: `-r .`.
//! - `cleaned-by`: alternatives `a b c | d e`, each a command name with arguments it must have (an
//!   argument written `^/dir` matches any argument that starts with `/dir`); the host needs one of
//!   them after the selected command.
//!
//! concept: embedded-code
//! what: Code of another language inside a file, reported by the host provider and parsed by that language.

use crate::{Params, Re};
use core_syntax::SyntaxNode;
use core_syntax::grammars::SHELL;
use core_syntax::strings;

/// A simple command of a script: its name and arguments, unquoted.
pub struct Cmd {
    pub name: String,
    pub args: Vec<String>,
}

impl Cmd {
    /// The names of the flags: `--long[=v]` is `long`, `-abc` is `a`, `b`, `c` (`--` and `-` are
    /// arguments).
    fn flags(&self) -> Vec<String> {
        self.args.iter().flat_map(|a| flag_names(a)).collect()
    }

    fn has_flag(&self, name: &str, times: usize) -> bool {
        self.flags().iter().filter(|f| *f == name).count() >= times
    }

    /// The arguments that are not flags, without the value of a flag in `skip` (the next word,
    /// unless the flag has `=value`).
    fn positionals(&self, skip: &[String]) -> Vec<&str> {
        let mut out = Vec::new();
        let mut args = self.args.iter();
        while let Some(a) = args.next() {
            let flags = flag_names(a);
            if flags.is_empty() {
                out.push(a.as_str());
            } else if !a.contains('=')
                && (skip.iter().any(|s| s == "*") || flags.iter().any(|f| skip.contains(f)))
            {
                args.next();
            }
        }
        out
    }
}

fn flag_names(arg: &str) -> Vec<String> {
    if arg == "--" || arg == "-" {
        Vec::new()
    } else if let Some(long) = arg.strip_prefix("--") {
        vec![long.split('=').next().unwrap_or("").to_string()]
    } else if let Some(short) = arg.strip_prefix('-') {
        short.chars().map(String::from).collect()
    } else {
        Vec::new()
    }
}

/// The commands of a shell tree, in order: the words of each `Command` node (a word is a token,
/// a string, a concatenation or a substitution; assignments and redirections are not words). A
/// command inside `$(...)` is a `Command` node of its own, found like the others.
pub fn shell_commands(root: &SyntaxNode) -> Vec<Cmd> {
    let shell = SHELL.get();
    let kind = |n| shell.kind_by_name(n);
    let command = kind("Command");
    let words = ["Word", "String", "Concat", "Subst"].map(kind);
    let rows: Vec<_> = shell
        .string_rows(shell.kind_by_name("Word").expect("Word"))
        .collect();
    let mut out = Vec::new();
    for node in root.descendants().filter(|n| Some(n.kind()) == command) {
        let mut words = (node.children_with_tokens())
            .filter(|e| words.contains(&Some(e.kind())))
            .map(|e| strings::concat_value(&rows, &e.to_string()));
        if let Some(name) = words.next() {
            out.push(Cmd {
                name,
                args: words.collect(),
            });
        }
    }
    out
}

/// The flags (leading `--` words) of an instruction's `words` when its script is a single command,
/// else `None`: the shape two adjacent instructions are compared by.
pub fn single_command_flags(
    cmds: &[Cmd],
    words: impl IntoIterator<Item = String>,
) -> Option<Vec<String>> {
    (cmds.len() == 1).then(|| {
        words
            .into_iter()
            .take_while(|w| w.starts_with("--"))
            .collect()
    })
}

/// Whether the script has a pipeline (`a | b`).
pub fn has_pipeline(root: &SyntaxNode) -> bool {
    let shell = SHELL.get();
    let pipeline = shell.kind_by_name("Pipeline");
    root.descendants().any(|n| Some(n.kind()) == pipeline)
}

/// The selection and the check of one rule, read from its params.
pub struct CommandCheck {
    program: Re,
    args: Vec<String>,
    needs: Vec<(String, String, usize)>,
    pinned: Option<Re>,
    unpinned: Option<Re>,
    skip_flag_args: Vec<String>,
    unless: Vec<String>,
    cleaned_by: Vec<Vec<String>>,
}

fn words(p: &mut Params, key: &str) -> Vec<String> {
    p.optional(key)
        .map(|v| v.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

fn regex(p: &mut Params, key: &str) -> Result<Option<Re>, String> {
    p.optional(key)
        .map(|v| Re::lazy(&v).map_err(|e| format!("param `{key}`: {e}")))
        .transpose()
}

impl CommandCheck {
    /// Reads the params above; the caller reads its own and calls `done`.
    pub fn read(p: &mut Params) -> Result<Self, String> {
        let program = p.text("program")?;
        Ok(CommandCheck {
            program: Re::lazy(&format!("^(?:{program})$"))
                .map_err(|e| format!("param `program`: {e}"))?,
            args: words(p, "args"),
            needs: words(p, "needs")
                .iter()
                .map(|f| {
                    let (name, times) = f.split_once('*').unwrap_or((f, "1"));
                    (
                        f.clone(),
                        name.trim_start_matches('-').to_string(),
                        times.parse().unwrap_or(1),
                    )
                })
                .collect(),
            pinned: regex(p, "pinned")?,
            unpinned: regex(p, "unpinned")?,
            skip_flag_args: words(p, "skip-flag-args"),
            unless: words(p, "unless"),
            cleaned_by: p
                .optional("cleaned-by")
                .map(|v| {
                    v.split('|')
                        .map(|a| a.split_whitespace().map(str::to_string).collect())
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// Whether a command of `cmds` is selected and breaks the check.
    pub fn hit(&self, cmds: &[Cmd]) -> bool {
        cmds.iter().enumerate().any(|(i, c)| {
            self.selected(c)
                && (self.broken(c) || (!self.cleaned_by.is_empty() && !self.cleaned(cmds, i + 1)))
        })
    }

    fn selected(&self, c: &Cmd) -> bool {
        self.program.is_match(&c.name)
            && (self.args.is_empty() || c.args.iter().any(|a| self.args.contains(a)))
    }

    fn exempt(&self, c: &Cmd) -> bool {
        self.unless.iter().any(|u| match u.strip_prefix('-') {
            Some(flag) => c.has_flag(flag.trim_start_matches('-'), 1),
            None => c.args.contains(u),
        })
    }

    fn broken(&self, c: &Cmd) -> bool {
        if self.exempt(c) {
            return false;
        }
        if !self.needs.is_empty() {
            // A spec also matches the literal argument (`-nv`, `-q=2`).
            return !self
                .needs
                .iter()
                .any(|(raw, f, n)| c.has_flag(f, *n) || c.args.contains(raw));
        }
        if self.pinned.is_some() || self.unpinned.is_some() {
            let positionals = c.positionals(&self.skip_flag_args);
            let after = positionals
                .iter()
                .position(|p| self.args.iter().any(|a| a == p));
            return positionals[after.map_or(0, |i| i + 1)..].iter().any(|p| {
                !self.pinned.as_ref().is_none_or(|r| r.is_match(p))
                    || self.unpinned.as_ref().is_some_and(|r| r.is_match(p))
            });
        }
        self.cleaned_by.is_empty()
    }

    fn cleaned(&self, cmds: &[Cmd], from: usize) -> bool {
        self.cleaned_by.iter().any(|alt| {
            cmds[from..].iter().any(|c| {
                alt.first().is_some_and(|n| *n == c.name)
                    && alt[1..].iter().all(|a| match a.strip_prefix('^') {
                        Some(prefix) => c.args.iter().any(|x| x.starts_with(prefix)),
                        None => c.args.contains(a),
                    })
            })
        })
    }
}
