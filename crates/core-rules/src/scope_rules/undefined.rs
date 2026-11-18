//! `undefined-variable`: a read or write that no scope binds, the language's globals do not list
//! and the rule's `globals` option does not name.

use super::cx::Cx;
use core_syntax::TextRange;

pub(super) fn run(cx: &Cx, extra: &[&str]) -> Vec<(TextRange, String)> {
    let (m, lang) = (cx.m, cx.lang);
    let dynamic = |scope| {
        let mut open = true;
        m.ancestors(scope)
            .take_while(|s| std::mem::replace(&mut open, !m.scope(*s).closed))
            .any(|s| m.scope(s).dynamic)
    };
    let mut hits = Vec::new();
    for r in m.unresolved() {
        let name = &*r.name;
        if (r.is_type() && !r.is_read())
            || r.is_probe()
            || r.is_tag()
            || (lang.skips_top() && m.scope(m.var_scope(r.scope)).parent.is_none())
            || lang.is_global(name, cx.file.path)
            || extra.contains(&name)
            || m.ancestors(r.scope)
                .any(|s| lang.scope_name(cx.file.grammar.name(m.scope(s).node.kind()), name))
            || dynamic(r.scope)
        {
            continue;
        }
        hits.push((r.range, name.to_string()));
    }
    hits
}
