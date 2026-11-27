//! Composer's autoload rules: the `autoload` and `autoload-dev` sections of `composer.json` and of
//! every package in `vendor/composer/installed.json` (PSR-4 and PSR-0 prefixes, class map paths,
//! autoloaded files).
//!
//! concept: package-locator
//! what: Finds the file of a qualified type name or an import from a project's package manifests (Composer autoload, import path roots).
//! tools: PHPStan class.notFound, buf IMPORT_USED
//! reads: file:symbols.rows

use super::{Located, Locator, existing};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

struct Prefix {
    prefix: String,
    dirs: Vec<PathBuf>,
    /// From the project's own `composer.json`: the project owns the whole namespace. A prefix of
    /// an installed package may share its namespace with a PHP extension or a phar (`MongoDB\\BSON`,
    /// `PHPStan\\`), so a file missing there proves nothing.
    own: bool,
}

/// The autoload rules of a project and its installed packages. A project without a manifest has
/// no prefix, so every name is `Unknown`.
#[derive(Default)]
pub struct Composer {
    psr4: Vec<Prefix>,
    psr0: Vec<Prefix>,
    classmap: Vec<PathBuf>,
    files: Vec<PathBuf>,
    vendor: PathBuf,
    expanded: OnceLock<Vec<PathBuf>>,
}

fn strings(v: Option<&Value>) -> Vec<&str> {
    match v {
        Some(Value::String(s)) => vec![s.as_str()],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

fn read(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

impl Composer {
    /// The rules of `root/composer.json` and of the packages `installed.json` lists, as far as
    /// those files exist.
    pub fn open(root: &Path) -> Composer {
        let mut c = Composer::default();
        let manifest = read(&root.join("composer.json"));
        if let Some(m) = &manifest {
            c.add_section(m, "autoload", root, true);
            c.add_section(m, "autoload-dev", root, true);
        }
        let vendor = manifest
            .as_ref()
            .and_then(|m| m.get("config")?.get("vendor-dir")?.as_str())
            .map_or_else(|| root.join("vendor"), |d| root.join(d));
        if let Some(installed) = read(&vendor.join("composer/installed.json")) {
            let packages = installed.get("packages").unwrap_or(&installed);
            for p in packages.as_array().into_iter().flatten() {
                let base = p.get("install-path").and_then(Value::as_str).map_or_else(
                    || vendor.join(p.get("name").and_then(Value::as_str).unwrap_or("")),
                    |rel| clean(&vendor.join("composer").join(rel)),
                );
                c.add_section(p, "autoload", &base, false);
            }
        }
        c.vendor = vendor;
        c
    }

    fn add_section(&mut self, owner: &Value, section: &str, base: &Path, own: bool) {
        let Some(auto) = owner.get(section) else {
            return;
        };
        for (key, list) in [("psr-4", &mut self.psr4), ("psr-0", &mut self.psr0)] {
            let Some(Value::Object(map)) = auto.get(key) else {
                continue;
            };
            for (prefix, dirs) in map {
                list.push(Prefix {
                    prefix: prefix.trim_start_matches('\\').to_string(),
                    dirs: strings(Some(dirs))
                        .into_iter()
                        .map(|d| base.join(d.trim_end_matches('/')))
                        .collect(),
                    own,
                });
            }
        }
        for p in strings(auto.get("classmap")) {
            self.classmap.push(base.join(p.trim_end_matches('/')));
        }
        for p in strings(auto.get("files")) {
            self.files.push(base.join(p));
        }
    }
}

/// PHP compares names without case, so a prefix matches in either case.
fn has_prefix(name: &str, prefix: &str) -> bool {
    name.is_char_boundary(prefix.len()) && name[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// `path` without `.` and `..` segments (so two spellings of a file are one path).
fn clean(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|e| e == "php") {
            out.push(p);
        }
    }
}

impl Locator for Composer {
    fn dependency_dir(&self) -> Option<PathBuf> {
        (!self.vendor.as_os_str().is_empty()).then(|| self.vendor.clone())
    }

    fn type_files(&self, name: &str) -> Located {
        let name = name.trim_start_matches('\\');
        let mut matched = false;
        let mut shared = false;
        let mut found = Vec::new();
        for p in self.psr4.iter().filter(|p| has_prefix(name, &p.prefix)) {
            matched = true;
            shared |= !p.own;
            let rel = format!("{}.php", name[p.prefix.len()..].replace('\\', "/"));
            found.extend(p.dirs.iter().filter_map(|d| existing(d, &rel)));
        }
        for p in self.psr0.iter().filter(|p| has_prefix(name, &p.prefix)) {
            matched = true;
            shared |= !p.own;
            let (space, class) = name.rsplit_once('\\').unwrap_or(("", name));
            let rel = if space.is_empty() {
                format!("{}.php", class.replace('_', "/"))
            } else {
                format!(
                    "{}/{}.php",
                    space.replace('\\', "/"),
                    class.replace('_', "/")
                )
            };
            found.extend(p.dirs.iter().filter_map(|d| existing(d, &rel)));
        }
        found.dedup();
        if !found.is_empty() {
            Located::Files(found)
        } else if matched && !shared {
            Located::Missing
        } else {
            Located::Unknown
        }
    }

    fn import_file(&self, _path: &str) -> Located {
        Located::Unknown
    }

    fn autoloaded(&self) -> Vec<PathBuf> {
        self.expanded
            .get_or_init(|| {
                let mut out: Vec<PathBuf> =
                    self.files.iter().filter(|f| f.is_file()).cloned().collect();
                for p in &self.classmap {
                    if p.is_dir() {
                        walk(p, &mut out);
                    } else if p.is_file() {
                        out.push(p.clone());
                    }
                }
                out.sort();
                out.dedup();
                out
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let w = |p: &str, text: &str| {
            let path = d.path().join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        w(
            "composer.json",
            r#"{"autoload":{"psr-4":{"App\\":"src/"},"files":["helpers.php"],"classmap":["legacy"]},
                "autoload-dev":{"psr-4":{"Tests\\":["tests/","more/"]}}}"#,
        );
        w("src/Models/User.php", "<?php");
        w("helpers.php", "<?php");
        w("legacy/Old.php", "<?php");
        w(
            "vendor/composer/installed.json",
            r#"{"packages":[{"name":"acme/lib","install-path":"../acme/lib",
                "autoload":{"psr-4":{"Acme\\Lib\\":"src"},"psr-0":{"Old_":"lib/"}}}]}"#,
        );
        w("vendor/acme/lib/src/Tool.php", "<?php");
        w("vendor/acme/lib/lib/Old/Thing.php", "<?php");
        d
    }

    #[test]
    fn names_map_to_files_through_psr_prefixes_of_the_project_and_its_packages() {
        let d = project();
        let c = Composer::open(d.path());
        let at = |p: &str| d.path().join(p);
        let files = |n: &str| c.type_files(n);
        assert_eq!(
            files("App\\Models\\User"),
            Located::Files(vec![at("src/Models/User.php")])
        );
        assert_eq!(
            files("Acme\\Lib\\Tool"),
            Located::Files(vec![at("vendor/acme/lib/src/Tool.php")])
        );
        assert_eq!(
            files("Old_Thing"),
            Located::Files(vec![at("vendor/acme/lib/lib/Old/Thing.php")])
        );
    }

    #[test]
    fn a_covered_name_without_a_file_is_missing_and_an_uncovered_one_unknown() {
        let d = project();
        let c = Composer::open(d.path());
        assert_eq!(c.type_files("App\\Nope"), Located::Missing);
        assert_eq!(c.type_files("Tests\\FooTest"), Located::Missing);
        assert_eq!(c.type_files("Elsewhere\\Foo"), Located::Unknown);
        assert_eq!(
            c.type_files("Acme\\Lib\\Nope"),
            Located::Unknown,
            "an installed package may share its namespace with an extension"
        );
        assert_eq!(c.type_files("Exception"), Located::Unknown);
        assert_eq!(
            Composer::open(&d.path().join("nothing")).type_files("App\\Foo"),
            Located::Unknown
        );
    }

    #[test]
    fn autoloaded_files_are_the_files_entries_and_the_class_map() {
        let d = project();
        let c = Composer::open(d.path());
        assert_eq!(
            c.autoloaded(),
            [
                d.path().join("helpers.php"),
                d.path().join("legacy/Old.php")
            ]
        );
    }

    #[test]
    fn a_name_written_in_another_case_still_finds_its_file() {
        let d = project();
        let c = Composer::open(d.path());
        assert_eq!(
            c.type_files("App\\models\\user"),
            Located::Files(vec![d.path().join("src/Models/User.php")])
        );
    }
}
