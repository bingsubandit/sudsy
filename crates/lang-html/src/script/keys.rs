//! Names a component defines twice: the groups of the options API (`props`, `computed`, `data`,
//! `methods`, `setup`), and the props of `defineProps` and models of `defineModel` against the
//! top-level names of `<script setup>`, where the template sees one name for both.
//!
//! concept: new component-option-checks
//! what: Names a component defines twice in its option groups, props and models.
//! tools: eslint-plugin-vue no-dupe-keys

use super::Cx;
use super::js::{self, args, calls, members, span, static_name, string};
use super::props::{DefineProps, Scan, entries, init_of};
use core_semantics::scopes::ScopeId;
use core_syntax::{SyntaxNode, SyntaxNodeExt, TextRange};
use lang_js::generated::kind as j;

const GROUPS: &[&str] = &["props", "computed", "data", "methods", "setup"];
const ROOT: ScopeId = ScopeId(0);

/// The names the groups of a component object define more than once, at the second.
pub(super) fn options(obj: &SyntaxNode, out: &mut Vec<(TextRange, String)>) {
    let mut used: Vec<String> = Vec::new();
    let mut add = |node: &SyntaxNode, name: String| {
        if used.contains(&name) {
            out.push((span(node), name));
        } else {
            used.push(name);
        }
    };
    for (_, m) in members(obj) {
        if !static_name(&m).is_some_and(|n| GROUPS.contains(&n.as_str())) {
            continue;
        }
        let Some(v) = m.value else { continue };
        match v.kind() {
            j::ARRAY_EXPRESSION => {
                for el in v.children() {
                    if let Some(name) = string(&el).filter(|n| !n.is_empty()) {
                        add(&el, name);
                    }
                }
            }
            j::OBJECT_EXPRESSION => object_keys(&v, &mut add),
            j::FUNCTION => returned(v.child(j::FUNCTION_BODY), &mut add),
            j::ARROW_FUNCTION_EXPRESSION => {
                let body = v.children().last();
                match body.as_ref().map(|b| b.kind()) {
                    Some(j::FUNCTION_BODY) => returned(body, &mut add),
                    Some(_) => {
                        if let Some(o) = body
                            .map(js::unwrap)
                            .filter(|o| o.kind() == j::OBJECT_EXPRESSION)
                        {
                            object_keys(&o, &mut add);
                        }
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }
}

/// The objects a function body returns at its top level.
fn returned(body: Option<SyntaxNode>, add: &mut impl FnMut(&SyntaxNode, String)) {
    for stmt in body.into_iter().flat_map(|b| b.children()) {
        if stmt.kind() == j::RETURN_STATEMENT
            && let Some(o) = stmt
                .children()
                .next()
                .map(js::unwrap)
                .filter(|o| o.kind() == j::OBJECT_EXPRESSION)
        {
            object_keys(&o, add);
        }
    }
}

/// The keys of an object; a setter is the key of its getter.
fn object_keys(obj: &SyntaxNode, add: &mut impl FnMut(&SyntaxNode, String)) {
    let mut getters_used: Vec<SyntaxNode> = Vec::new();
    for (_, m) in members(obj) {
        let Some(name) = static_name(&m).filter(|n| !n.is_empty()) else {
            continue;
        };
        if m.accessor == Some("set") {
            let getter = members(obj).find(|(g, gm)| {
                gm.accessor == Some("get")
                    && !getters_used.contains(g)
                    && static_name(gm).is_some_and(|n| n == name)
            });
            if let Some((g, _)) = getter {
                getters_used.push(g);
                continue;
            }
        }
        add(&m.key, name);
    }
}

/// The references of the names a pattern declares.
fn pattern_refs(cx: &Cx, pattern: &SyntaxNode, out: &mut Vec<TextRange>) {
    match pattern.kind() {
        j::BINDING_IDENTIFIER => {
            let model = cx.model();
            if let Some(sym) = js::first(pattern).and_then(|n| model.own(ROOT, &n)) {
                out.extend(model.refs_of(sym).map(|r| r.range));
            }
        }
        j::OBJECT_PATTERN => {
            for p in pattern.children() {
                let value = if p.kind() == j::BINDING_PROPERTY {
                    p.children()
                        .last()
                        .filter(|_| p.children().count() > 1)
                        .or_else(|| p.children().next())
                } else {
                    p.children().next()
                };
                if let Some(v) = value {
                    pattern_refs(cx, &v, out);
                }
            }
        }
        j::ASSIGNMENT_PATTERN => {
            if let Some(left) = pattern.children().next() {
                // `{ a = 1 }`: the left is the key node that holds the binding
                let left = left.child(j::BINDING_IDENTIFIER).unwrap_or(left);
                pattern_refs(cx, &left, out);
            }
        }
        j::IDENTIFIER_NAME => {
            if let Some(b) = pattern.child(j::BINDING_IDENTIFIER) {
                pattern_refs(cx, &b, out);
            }
        }
        j::BINDING_REST_ELEMENT => {
            if let Some(b) = pattern.children().next() {
                pattern_refs(cx, &b, out);
            }
        }
        _ => {}
    }
}

fn in_refs(n: &SyntaxNode, refs: &[TextRange]) -> bool {
    refs.contains(&span(n))
}

/// The prop that `node` reads from the props object, `props.a`, `props['a']`, `toRef(props, 'a')`.
fn aliased_prop(node: &SyntaxNode, refs: &[TextRange]) -> Option<String> {
    let e = js::unwrap(node.clone());
    let e = if e.kind() == j::CHAIN_EXPRESSION {
        js::unwrap(e.children().next()?)
    } else {
        e
    };
    match e.kind() {
        j::STATIC_MEMBER_EXPRESSION | j::COMPUTED_MEMBER_EXPRESSION => {
            let mut parts = e.children();
            let object = js::unwrap(parts.next()?);
            let property = parts.next()?;
            if !in_refs(&object, refs) {
                return None;
            }
            if e.kind() == j::STATIC_MEMBER_EXPRESSION {
                js::first(&property)
            } else {
                string(&property)
            }
        }
        j::CALL_EXPRESSION if calls(&e, "toRef") => {
            let mut a = args(&e);
            let target = a.next()?;
            in_refs(&target, refs)
                .then(|| a.next().and_then(|k| string(&k)))
                .flatten()
        }
        _ => None,
    }
}

fn is_props_object(node: &SyntaxNode, refs: &[TextRange]) -> bool {
    in_refs(node, refs)
        || calls(node, "toRefs") && args(node).next().is_some_and(|a| in_refs(&a, refs))
}

/// Does the declarator bind `name` to a different prop (`const a = toRef(props, 'b')`)?
fn aliases_other(d: &SyntaxNode, name: &str, refs: &[TextRange]) -> bool {
    let (Some(init), Some(id)) = (init_of(d), d.children().next()) else {
        return false;
    };
    match id.kind() {
        j::BINDING_IDENTIFIER if js::first(&id).as_deref() == Some(name) => {
            aliased_prop(&init, refs).is_some_and(|a| a != name)
        }
        j::OBJECT_PATTERN if is_props_object(&init, refs) => entries(&id)
            .into_iter()
            .find(|e| e.binding.as_deref() == Some(name))
            .is_some_and(|e| e.key.is_some_and(|k| k != name)),
        _ => false,
    }
}

fn inside_initializer(d: &SyntaxNode, refs: &[TextRange]) -> bool {
    init_of(d).is_some_and(|i| {
        let r = span(&i);
        refs.iter().any(|x| r.contains_range(*x))
    })
}

/// A declaration of `name` in the top scope of the script, except those that read from `refs`.
fn clash(cx: &Cx, name: &str, refs: &[TextRange], alias_checked: bool) -> Option<SyntaxNode> {
    let model = cx.model();
    let sym = model.symbol(model.own(ROOT, name)?);
    let decls: Vec<SyntaxNode> = sym.decls.iter().map(|d| d.node(cx.root)).collect();
    let reads = decls.iter().any(|d| {
        d.kind() == j::VARIABLE_DECLARATOR
            && !(alias_checked && aliases_other(d, name, refs))
            && inside_initializer(d, refs)
    });
    (!reads).then(|| decls.into_iter().next()).flatten()
}

/// The props and models of `<script setup>` that a top-level name of the script repeats.
pub(super) fn setup(cx: &Cx, scan: &Scan, out: &mut Vec<(TextRange, String)>) {
    let mut names: Vec<&str> = Vec::new();
    let mut refs: Vec<TextRange> = Vec::new();
    let mut reported: Vec<SyntaxNode> = Vec::new();
    for dp in &scan.props {
        refs = props_refs(cx, dp);
        let pattern = dp.decl.as_ref().and_then(|d| d.children().next());
        let renamed: Vec<String> = pattern
            .filter(|p| p.kind() == j::OBJECT_PATTERN)
            .map(|p| entries(&p))
            .unwrap_or_default()
            .into_iter()
            .filter(|e| !e.default && e.key != e.binding && e.key.is_some() && e.binding.is_some())
            .filter_map(|e| e.key)
            .collect();
        for name in dp.props.iter().filter_map(|p| p.name.as_deref()) {
            names.push(name);
            if renamed.iter().any(|r| r == name) {
                continue;
            }
            if let Some(decl) = clash(cx, name, &refs, true) {
                out.push((span(&decl), name.to_string()));
                reported.push(decl);
            }
        }
    }
    for m in &scan.models {
        if names.contains(&m.name.as_str()) {
            if !m.decl.as_ref().is_some_and(|d| reported.contains(d)) {
                out.push((
                    span(m.name_node.as_ref().unwrap_or(&m.call)),
                    m.name.clone(),
                ));
            }
            continue;
        }
        names.push(&m.name);
        let mut model_refs = refs.clone();
        if let Some(p) = m.decl.as_ref().and_then(|d| d.children().next()) {
            pattern_refs(cx, &p, &mut model_refs);
        }
        model_refs.push(span(&m.call));
        if let Some(decl) = clash(cx, &m.name, &model_refs, false) {
            out.push((span(&decl), m.name.clone()));
        }
    }
}

/// The references to the props object, and the `defineProps` call itself.
fn props_refs(cx: &Cx, dp: &DefineProps) -> Vec<TextRange> {
    let mut refs = Vec::new();
    if let Some(p) = dp.decl.as_ref().and_then(|d| d.children().next()) {
        pattern_refs(cx, &p, &mut refs);
    }
    refs.push(span(&dp.call));
    refs
}
