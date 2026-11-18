//! I001: an import block that isort, as ruff runs it with its defaults, would write differently.
//!
//! A block is a run of import statements with no code between them; a comment line above an import stays with it, after a blank line. Its
//! imports are grouped into sections (future, standard library, third party, first party,
//! relative), `import x` before `from x import y` in each, sorted without regard to case, the
//! members of a `from` import sorted by constants, classes, variables; `from` imports of one
//! module are one statement (`as` aliases and `*` apart), and a line over the width is wrapped in
//! parentheses. The rule writes the block that way and compares it with the text.
//!
//! concept: sorted-imports

use super::Check;
use super::lints::bodies;
use super::literal::natural;
use super::tree::{span, tokens};
use crate::generated::kind as k;
use crate::semantic::model;
use crate::text::code_text;
use core_rules::RuleContext;
use core_syntax::{SyntaxNode, TextSize};
use std::cmp::Ordering;
use std::path::Path;

/// Top-level names of the standard library.
const STDLIB: &[&str] = &[
    "__future__",
    "_abc",
    "_aix_support",
    "_ast",
    "_asyncio",
    "_bisect",
    "_blake2",
    "_bz2",
    "_codecs",
    "_codecs_cn",
    "_codecs_hk",
    "_codecs_iso2022",
    "_codecs_jp",
    "_codecs_kr",
    "_codecs_tw",
    "_collections",
    "_collections_abc",
    "_compat_pickle",
    "_compression",
    "_contextvars",
    "_crypt",
    "_csv",
    "_ctypes",
    "_curses",
    "_curses_panel",
    "_datetime",
    "_dbm",
    "_decimal",
    "_elementtree",
    "_frozen_importlib",
    "_frozen_importlib_external",
    "_functools",
    "_gdbm",
    "_hashlib",
    "_heapq",
    "_imp",
    "_io",
    "_json",
    "_locale",
    "_lsprof",
    "_lzma",
    "_markupbase",
    "_md5",
    "_msi",
    "_multibytecodec",
    "_multiprocessing",
    "_opcode",
    "_operator",
    "_osx_support",
    "_overlapped",
    "_pickle",
    "_posixshmem",
    "_posixsubprocess",
    "_py_abc",
    "_pydatetime",
    "_pydecimal",
    "_pyio",
    "_pylong",
    "_queue",
    "_random",
    "_scproxy",
    "_sha1",
    "_sha2",
    "_sha3",
    "_signal",
    "_sitebuiltins",
    "_socket",
    "_sqlite3",
    "_sre",
    "_ssl",
    "_stat",
    "_statistics",
    "_string",
    "_strptime",
    "_struct",
    "_symtable",
    "_thread",
    "_threading_local",
    "_tkinter",
    "_tokenize",
    "_tracemalloc",
    "_typing",
    "_uuid",
    "_warnings",
    "_weakref",
    "_weakrefset",
    "_winapi",
    "_zoneinfo",
    "abc",
    "aifc",
    "annotationlib",
    "antigravity",
    "argparse",
    "array",
    "ast",
    "asyncio",
    "atexit",
    "audioop",
    "base64",
    "bdb",
    "binascii",
    "bisect",
    "builtins",
    "bz2",
    "cProfile",
    "calendar",
    "cgi",
    "cgitb",
    "chunk",
    "cmath",
    "cmd",
    "code",
    "codecs",
    "codeop",
    "collections",
    "colorsys",
    "compileall",
    "compression",
    "concurrent",
    "configparser",
    "contextlib",
    "contextvars",
    "copy",
    "copyreg",
    "crypt",
    "csv",
    "ctypes",
    "curses",
    "dataclasses",
    "datetime",
    "dbm",
    "decimal",
    "difflib",
    "dis",
    "doctest",
    "email",
    "encodings",
    "ensurepip",
    "enum",
    "errno",
    "faulthandler",
    "fcntl",
    "filecmp",
    "fileinput",
    "fnmatch",
    "fractions",
    "ftplib",
    "functools",
    "gc",
    "genericpath",
    "getopt",
    "getpass",
    "gettext",
    "glob",
    "graphlib",
    "grp",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    "idlelib",
    "imaplib",
    "imghdr",
    "importlib",
    "inspect",
    "io",
    "ipaddress",
    "itertools",
    "json",
    "keyword",
    "lib2to3",
    "linecache",
    "locale",
    "logging",
    "lzma",
    "mailbox",
    "mailcap",
    "marshal",
    "math",
    "mimetypes",
    "mmap",
    "modulefinder",
    "msilib",
    "msvcrt",
    "multiprocessing",
    "netrc",
    "nis",
    "nntplib",
    "nt",
    "ntpath",
    "nturl2path",
    "numbers",
    "opcode",
    "operator",
    "optparse",
    "os",
    "ossaudiodev",
    "pathlib",
    "pdb",
    "pickle",
    "pickletools",
    "pipes",
    "pkgutil",
    "platform",
    "plistlib",
    "poplib",
    "posix",
    "posixpath",
    "pprint",
    "profile",
    "pstats",
    "pty",
    "pwd",
    "py_compile",
    "pyclbr",
    "pydoc",
    "pydoc_data",
    "pyexpat",
    "queue",
    "quopri",
    "random",
    "re",
    "readline",
    "reprlib",
    "resource",
    "rlcompleter",
    "runpy",
    "sched",
    "secrets",
    "select",
    "selectors",
    "shelve",
    "shlex",
    "shutil",
    "signal",
    "site",
    "smtplib",
    "sndhdr",
    "socket",
    "socketserver",
    "spwd",
    "sqlite3",
    "sre_compile",
    "sre_constants",
    "sre_parse",
    "ssl",
    "stat",
    "statistics",
    "string",
    "stringprep",
    "struct",
    "subprocess",
    "sunau",
    "symtable",
    "sys",
    "sysconfig",
    "syslog",
    "tabnanny",
    "tarfile",
    "telnetlib",
    "tempfile",
    "termios",
    "textwrap",
    "this",
    "threading",
    "time",
    "timeit",
    "tkinter",
    "token",
    "tokenize",
    "tomllib",
    "trace",
    "traceback",
    "tracemalloc",
    "tty",
    "turtle",
    "turtledemo",
    "types",
    "typing",
    "unicodedata",
    "unittest",
    "urllib",
    "uu",
    "uuid",
    "venv",
    "warnings",
    "wave",
    "weakref",
    "webbrowser",
    "winreg",
    "winsound",
    "wsgiref",
    "xdrlib",
    "xml",
    "xmlrpc",
    "zipapp",
    "zipfile",
    "zipimport",
    "zlib",
    "zoneinfo",
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    Future,
    Stdlib,
    Third,
    First,
    Local,
}

/// Is `top` a module of the project: a folder or file next to the file, in the project's root or
/// in its `src` folder?
fn first_party(path: &Path, top: &str) -> bool {
    let Some(dir) = path.parent() else {
        return false;
    };
    let marked = |d: &Path| {
        [
            "pyproject.toml",
            "ruff.toml",
            ".ruff.toml",
            "sudsy.yaml",
            ".git",
        ]
        .iter()
        .any(|m| d.join(m).exists())
    };
    let root = dir.ancestors().find(|d| marked(d)).unwrap_or(dir);
    [root.to_path_buf(), root.join("src"), dir.to_path_buf()]
        .iter()
        .any(|d| d.join(top).is_dir() || d.join(format!("{top}.py")).is_file())
}

fn section(path: &Path, level: usize, module: &str) -> Section {
    let top = module.split('.').next().unwrap_or(module);
    if level > 0 {
        Section::Local
    } else if top == "__future__" {
        Section::Future
    } else if STDLIB.binary_search(&top).is_ok() {
        Section::Stdlib
    } else if first_party(path, top) {
        Section::First
    } else {
        Section::Third
    }
}

/// One line of the written block (before the wrapping).
struct Entry {
    section: Section,
    /// `from m import ...` rather than `import m`.
    from: bool,
    level: usize,
    module: String,
    /// The `from` members, or the `as` name of an `import`.
    members: Vec<(String, Option<String>)>,
    star: bool,
    /// A trailing comma in parentheses: the statement stays one member to a line.
    magic: bool,
    comment: Option<String>,
    /// Comment lines above the statement.
    atop: Vec<String>,
}

impl Entry {
    fn plain(&self) -> bool {
        self.from && !self.star && self.members.iter().all(|(_, a)| a.is_none())
    }
}

fn member_rank(name: &str) -> u8 {
    let cased = name.chars().any(char::is_alphabetic);
    if name.chars().count() > 1 && cased && !name.chars().any(char::is_lowercase) {
        0
    } else if name.starts_with(char::is_uppercase) {
        1
    } else {
        2
    }
}

fn member_cmp(a: &(String, Option<String>), b: &(String, Option<String>)) -> Ordering {
    member_rank(&a.0)
        .cmp(&member_rank(&b.0))
        .then_with(|| natural(&a.0.to_lowercase(), &b.0.to_lowercase()))
        .then_with(|| natural(&a.0, &b.0))
        .then_with(|| a.1.cmp(&b.1))
}

fn module_cmp(a: &str, b: &str) -> Ordering {
    natural(&a.to_lowercase(), &b.to_lowercase()).then_with(|| natural(a, b))
}

fn entry_cmp(a: &Entry, b: &Entry) -> Ordering {
    a.section
        .cmp(&b.section)
        .then(a.from.cmp(&b.from))
        .then_with(|| b.level.cmp(&a.level))
        .then_with(|| module_cmp(&a.module, &b.module))
        .then_with(|| match (a.members.first(), b.members.first()) {
            (Some(x), Some(y)) => member_cmp(x, y),
            (x, y) => x.is_some().cmp(&y.is_some()),
        })
}

const WIDTH: usize = 88;

fn member_text((name, alias): &(String, Option<String>)) -> String {
    match alias {
        Some(a) => format!("{name} as {a}"),
        None => name.clone(),
    }
}

fn render(e: &Entry, indent: &str, out: &mut Vec<String>) {
    let comment = e.comment.as_deref().map(|c| format!("  {c}"));
    let tail = comment.clone().unwrap_or_default();
    if !e.from {
        let name = match e.members.first().and_then(|(_, a)| a.as_ref()) {
            Some(a) => format!("{} as {a}", e.module),
            None => e.module.clone(),
        };
        out.push(format!("{indent}import {name}{tail}"));
        return;
    }
    let head = format!("from {}{} import ", ".".repeat(e.level), e.module);
    if e.star {
        out.push(format!("{indent}{head}*{tail}"));
        return;
    }
    let members: Vec<String> = e.members.iter().map(member_text).collect();
    let line = format!("{indent}{head}{}", members.join(", "));
    if !e.magic && line.chars().count() <= WIDTH {
        out.push(format!("{line}{tail}"));
        return;
    }
    out.push(format!("{indent}{head}({}", comment.unwrap_or_default()));
    out.extend(members.iter().map(|m| format!("{indent}    {m},")));
    out.push(format!("{indent})"));
}

/// What a statement of the block says: its entries, or `Err(flag)` when the block need not be
/// written out (`true`: it needs changes whatever the rest says).
fn parse(ctx: &RuleContext<'_>, stmts: &[SyntaxNode], text: &str) -> Result<Vec<Entry>, bool> {
    let Some(cx) = model(ctx.file) else {
        return Err(false);
    };
    let mut out: Vec<Entry> = Vec::new();
    let mut before = None;
    for stmt in stmts {
        let range = span(ctx, stmt);
        let atop: Vec<String> = before.map_or_else(Vec::new, |from: usize| {
            text[from..usize::from(range.start())]
                .split('\n')
                .skip(1)
                .map(str::trim)
                .filter(|l| l.starts_with('#'))
                .map(str::to_owned)
                .collect()
        });
        before = Some(usize::from(range.end()));
        let (start, end) = (usize::from(range.start()), usize::from(range.end()));
        let source = &text[start..end];
        if source.contains('#') {
            return Err(false);
        }
        let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
        let rest = text[end..line_end].trim();
        let comment = if rest.starts_with('#') {
            Some(rest.to_owned())
        } else if rest.is_empty() {
            None
        } else {
            return Err(true);
        };
        if comment.as_deref().is_some_and(|c| c.contains("isort:")) {
            return Err(false);
        }
        let magic = {
            let kinds: Vec<_> = tokens(stmt).map(|t| t.kind()).collect();
            kinds
                .iter()
                .position(|x| *x == k::RPAR)
                .is_some_and(|i| i > 0 && kinds[i - 1] == k::COMMA)
        };
        if stmt.kind() == k::STMT_IMPORT {
            for alias in stmt.children().filter(|a| a.kind() == k::ALIAS) {
                let name = cx
                    .slot(&alias, "name")
                    .map(|n| code_text(&n))
                    .unwrap_or_default();
                let asname = cx.slot(&alias, "asname").map(|n| code_text(&n));
                out.push(Entry {
                    section: section(ctx.file.path, 0, &name),
                    from: false,
                    level: 0,
                    module: name.clone(),
                    members: vec![(name, asname)],
                    star: false,
                    magic: false,
                    comment: comment.clone(),
                    atop: atop.clone(),
                });
            }
            continue;
        }
        let level = tokens(stmt)
            .map(|t| match t.kind() {
                k::DOT => 1,
                k::ELLIPSIS => 3,
                _ => 0,
            })
            .sum();
        let module = cx
            .slot(stmt, "module")
            .map(|m| code_text(&m))
            .unwrap_or_default();
        let members: Vec<(String, Option<String>)> = stmt
            .children()
            .filter(|a| a.kind() == k::ALIAS)
            .map(|a| {
                (
                    cx.slot(&a, "name")
                        .map(|n| code_text(&n))
                        .unwrap_or_default(),
                    cx.slot(&a, "asname").map(|n| code_text(&n)),
                )
            })
            .collect();
        let star = members.is_empty() || members.iter().any(|(n, _)| n == "*");
        let sec = section(ctx.file.path, level, &module);
        let (aliased, mut plain): (Vec<_>, Vec<_>) = members
            .into_iter()
            .filter(|(n, _)| n != "*")
            .partition(|(_, a)| a.is_some());
        let make = |members: Vec<(String, Option<String>)>, star: bool, magic: bool| Entry {
            section: sec,
            from: true,
            level,
            module: module.clone(),
            members,
            star,
            magic,
            comment: comment.clone(),
            atop: atop.clone(),
        };
        if star {
            out.push(make(Vec::new(), true, false));
        }
        for m in aliased {
            out.push(make(vec![m], false, false));
        }
        if !plain.is_empty() {
            plain.sort_by(member_cmp);
            out.push(make(plain, false, magic));
        }
    }
    Ok(out)
}

/// The block as it should be written, or `Err(flag)` as in [`parse`].
fn written(
    ctx: &RuleContext<'_>,
    stmts: &[SyntaxNode],
    text: &str,
    indent: &str,
) -> Result<Vec<String>, bool> {
    let mut entries = parse(ctx, stmts, text)?;
    // Statements to combine, and imports written twice, need changes whatever else is right.
    for (i, e) in entries.iter().enumerate() {
        let same = entries[..i].iter().any(|o| {
            o.from == e.from
                && o.level == e.level
                && o.module == e.module
                && o.members == e.members
                && o.star == e.star
        });
        let combine = e.plain()
            && entries[..i]
                .iter()
                .any(|o| o.plain() && o.level == e.level && o.module == e.module);
        if same || combine {
            return Err(true);
        }
    }
    entries.sort_by(entry_cmp);
    let mut lines = Vec::new();
    let mut last = None;
    for e in &entries {
        if last.is_some_and(|s| s != e.section) {
            lines.push(String::new());
        }
        last = Some(e.section);
        if !e.atop.is_empty() {
            if lines.last().is_some_and(|l| !l.is_empty()) {
                lines.push(String::new());
            }
            lines.extend(e.atop.iter().map(|c| format!("{indent}{c}")));
        }
        render(e, indent, &mut lines);
    }
    Ok(lines)
}

/// Is the node a block of imports' follower that wants two blank lines before it?
fn wants_two(n: &SyntaxNode) -> bool {
    matches!(n.kind(), k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF)
}

fn check_block(
    c: &Check,
    n: &SyntaxNode,
    stmts: &[SyntaxNode],
    next: Option<&SyntaxNode>,
    ctx: &mut RuleContext<'_>,
) {
    let text = ctx.file.text;
    let (Some(first), Some(last)) = (stmts.first(), stmts.last()) else {
        return;
    };
    let start = usize::from(span(ctx, first).start());
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let indent = &text[line_start..start];
    if !indent.chars().all(char::is_whitespace) {
        return;
    }
    let end = usize::from(span(ctx, last).end());
    let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
    if text[line_start..line_end].contains("isort:") {
        return;
    }
    let wrong = match written(ctx, stmts, text, indent) {
        Err(flag) => flag,
        Ok(lines) => {
            let actual = text[line_start..line_end].lines().map(str::trim_end);
            let same = actual.eq(lines.iter().map(|l| l.trim_end()));
            // After a block at the top of the file: one blank line, two before a definition.
            let blanks = n.kind() == k::MOD_MODULE
                && next.is_some_and(|nx| {
                    let rest = &text[line_end..];
                    let count = rest
                        .split('\n')
                        .skip(1)
                        .take_while(|l| l.trim().is_empty())
                        .count();
                    let code = rest.split('\n').skip(1).find(|l| !l.trim().is_empty());
                    // Comments between the block and a definition: either number is fine.
                    let comment = code.is_some_and(|l| l.trim_start().starts_with('#'));
                    let ok = match (wants_two(nx), comment) {
                        (true, true) => matches!(count, 1 | 2),
                        (true, false) => count == 2,
                        (false, _) => count == 1,
                    };
                    code.is_some() && !ok
                });
            !same || blanks
        }
    };
    if wrong {
        let at = TextSize::try_from(start).unwrap_or_default();
        let to = TextSize::try_from(end).unwrap_or_default();
        c.hit(ctx, core_syntax::TextRange::new(at, to), "");
    }
}

/// I001.
pub(super) fn unsorted_imports(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    for body in bodies(n) {
        let mut at = 0;
        while at < body.len() {
            if !matches!(body[at].kind(), k::STMT_IMPORT | k::STMT_IMPORT_FROM) {
                at += 1;
                continue;
            }
            let mut end = at + 1;
            while end < body.len()
                && matches!(body[end].kind(), k::STMT_IMPORT | k::STMT_IMPORT_FROM)
            {
                end += 1;
            }
            check_block(c, n, &body[at..end], body.get(end), ctx);
            at = end;
        }
    }
}
