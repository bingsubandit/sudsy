//! The import order of google-java-format: static imports first, then the others, each group in
//! ASCII order of the imported name, no duplicates, one blank line between the groups. Imports
//! with a comment between them are left as written.

use crate::generated::kind as k;
use core_syntax::SyntaxNode;

/// The source with its import block sorted, or `None` when it is already so (or not sortable).
pub fn sort_imports(tree: &SyntaxNode, text: &str) -> Option<String> {
    let mut items: Vec<(usize, usize)> = Vec::new();
    for n in tree
        .descendants()
        .filter(|n| n.kind() == k::IMPORT_DECLARATION)
    {
        let r = n.text_range();
        let (s, e) = (usize::from(r.start()), usize::from(r.end()));
        let piece = &text[s..e];
        if !piece.trim().starts_with("import") || piece.contains("/*") || piece.contains("//") {
            return None; // a comment belongs to the import
        }
        let lead = piece.len() - piece.trim_start().len();
        items.push((s + lead, s + piece.trim_end().len()));
    }
    let (first, last) = (items.first()?.0, items.last()?.1);
    if items
        .windows(2)
        .any(|w| !text[w[0].1..w[1].0].trim().is_empty())
    {
        return None;
    }
    let name = |s: &str| -> (bool, String) {
        let body = s.trim().strip_prefix("import").unwrap_or(s).trim_start();
        let (is_static, body) = match body.strip_prefix("static") {
            Some(r) if r.starts_with(char::is_whitespace) => (true, r),
            _ => (false, body),
        };
        let name: String = body
            .chars()
            .filter(|c| !c.is_whitespace() && *c != ';')
            .collect();
        (is_static, name)
    };
    let mut keyed: Vec<(bool, String, &str)> = items
        .iter()
        .map(|&(s, e)| {
            let (st, n) = name(&text[s..e]);
            (!st, n, &text[s..e])
        })
        .collect();
    keyed.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    keyed.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    let mut out = String::new();
    for (i, (non_static, _, piece)) in keyed.iter().enumerate() {
        if i > 0 {
            out.push('\n');
            if *non_static != keyed[i - 1].0 {
                out.push('\n');
            }
        }
        out.push_str(piece);
    }
    (out != text[first..last]).then(|| format!("{}{}{}", &text[..first], out, &text[last..]))
}

#[cfg(test)]
mod tests {
    use provider_api::{LanguageProvider, SourceFile};

    fn sorted(src: &str) -> Option<String> {
        let tree = crate::provider()
            .parse(&SourceFile::new("A.java", 1, src))
            .unwrap()
            .tree;
        super::sort_imports(&tree, src)
    }

    #[test]
    fn static_first_ascii_order_without_duplicates() {
        let src = "package a;\nimport b.C;\nimport static d.E.f;\nimport a.B;\nimport b.C;\n\nclass A {}\n";
        assert_eq!(
            sorted(src).unwrap(),
            "package a;\nimport static d.E.f;\n\nimport a.B;\nimport b.C;\n\nclass A {}\n"
        );
        assert_eq!(sorted("import a.B;\nimport b.C;\nclass A {}\n"), None);
        assert_eq!(sorted("import b.C;\n// x\nimport a.B;\n"), None);
    }
}
