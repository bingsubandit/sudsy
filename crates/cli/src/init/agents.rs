//! The Sudsy block of the repo's agent instructions (`AGENTS.md`, or `CLAUDE.md` when only that
//! exists): how to run Sudsy. Written on request (`--agents`, or the question of `sudsy init`), once.

use anyhow::Result;
use std::path::Path;

const START: &str = "<!-- sudsy:start -->";
const END: &str = "<!-- sudsy:end -->";

/// Paragraphs, not a list: the formatter writes list markers by the configured indent width.
const BLOCK: &str = "## Sudsy

This repo is formatted and linted with Sudsy (`sudsy.yaml`).

Check: `sudsy check` (exit 1 on an error or an unformatted file). Each finding shows its line and its fixes; a fix that needs a decision has an id (`fix ab12cd (unsafe)`). A report of more than 100 findings is one line per rule; `sudsy check --rule <id>` lists one rule.

Fix: `sudsy fix --unsafe` applies every safe and unsafe fix, unless one must be checked first; `sudsy fix --apply <id>,<id>` applies only the fixes named (suggestions too). Both then format and print what is left. Never edit by hand what `sudsy fix` does.

Explain a rule: `sudsy rules <id>`. Silence one line: `sudsy-disable-next-line <id>` in a comment; change a level in `sudsy.yaml`.
";

/// The instruction files a repository can have, in the order `sudsy init` lists them.
pub const FILES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// The file `--agents` writes in `dir`: `CLAUDE.md` when only that exists.
pub fn default_file(dir: &Path) -> &'static str {
    if !dir.join(FILES[0]).exists() && dir.join(FILES[1]).exists() {
        FILES[1]
    } else {
        FILES[0]
    }
}

/// `text` with the block appended; `None` when it already has it.
pub fn with_block(text: &str) -> Option<String> {
    if text.contains(START) {
        return None;
    }
    let sep = match text {
        "" => "",
        t if t.ends_with("\n\n") => "",
        t if t.ends_with('\n') => "\n",
        _ => "\n\n",
    };
    Some(format!("{text}{sep}{START}\n\n{BLOCK}\n{END}\n"))
}

/// Adds the block to the instruction file in `dir`. Returns the file and whether it changed.
pub fn add(dir: &Path) -> Result<(&'static str, bool)> {
    let name = default_file(dir);
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let Some(new) = with_block(&text) else {
        return Ok((name, false));
    };
    std::fs::write(&path, new)?;
    Ok((name, true))
}
