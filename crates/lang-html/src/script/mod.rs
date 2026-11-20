//! The script of a Vue component, read as JavaScript. The host reports `<script>` and
//! `<script setup>` as one embedded region (`region`), the engine parses it with the JS
//! provider, and the `script` family reads that tree and the scope model of `lang-js` for the
//! rules of eslint-plugin-vue that need the script. One analysis per file (`analyze`) finds what
//! every rule of the family reports; a rule picks its own. Param `check`:
//! - `default-prop`: a prop that is not required, has no default and is not a `Boolean`;
//!   declared as an object (`defineProps({})`, the `props` option) or as a type, where only a
//!   component that sets defaults (`withDefaults`, a destructure) is checked;
//! - `prop-types`: a prop declared without a type (the array form, an object with no `type` or
//!   `validator`);
//! - `dupe-keys`: a name defined twice in `props`, `computed`, `data`, `methods` and `setup`; or a
//!   prop or model of `<script setup>` that a top-level name of the script repeats.
//!
//! The message has `{name}`.
//!
//! concept: embedded-code

mod js;
mod keys;
mod props;
mod region;

use core_rules::checks::FileCache;
use core_rules::{Family, FileContext, Params, Rule, RuleContext, RuleMeta};
use core_semantics::scopes::{Model, ScopeId};
use core_syntax::{SyntaxNode, TextRange};
use std::cell::OnceCell;
use std::rc::Rc;

pub const FAMILY: Family = Family {
    name: "script",
    build,
};

#[derive(Clone, Copy, PartialEq)]
enum Check {
    DefaultProp,
    PropTypes,
    DupeKeys,
}

struct Hit {
    check: Check,
    range: TextRange,
    name: String,
}

struct Script {
    meta: &'static RuleMeta,
    check: Check,
    message: String,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let check = p.choice(
        "check",
        true,
        &[
            ("default-prop", Check::DefaultProp),
            ("prop-types", Check::PropTypes),
            ("dupe-keys", Check::DupeKeys),
        ],
    )?;
    let rule = Script {
        meta,
        check,
        message: p.text("message")?,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Script {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        if file.embedded.is_empty() {
            return;
        }
        for hit in hits(file).iter().filter(|h| h.check == self.check) {
            ctx.report(hit.range, self.message.replace("{name}", &hit.name));
        }
    }

    fn reads_embedded(&self) -> bool {
        true
    }
}

thread_local! {
    static CACHE: FileCache<Vec<Hit>> = const { FileCache::new() };
}

/// What the rules of the family report in this file, analysed once.
fn hits(file: &FileContext<'_>) -> Rc<Vec<Hit>> {
    CACHE.with(|cache| cache.get(file, || analyze(file)))
}

/// The script's tree, where `<script setup>` is, and the scope model (built when a check asks).
pub(super) struct Cx<'a> {
    root: &'a SyntaxNode,
    setup: Option<TextRange>,
    model: OnceCell<Model>,
}

impl Cx<'_> {
    fn model(&self) -> &Model {
        self.model.get_or_init(|| lang_js::bind_embedded(self.root))
    }

    fn in_setup(&self, n: &SyntaxNode) -> bool {
        self.setup.is_some_and(|s| s.contains_range(js::span(n)))
    }

    /// The only declaration of a top-level name, as a declaring node.
    fn only_decl(&self, name: &str, ty: bool) -> Option<SyntaxNode> {
        let model = self.model();
        let sym = model.symbol(model.lookup(ScopeId(0), name, ty)?);
        (sym.decls.len() == 1).then(|| sym.decls[0].node(self.root))
    }
}

fn analyze(file: &FileContext<'_>) -> Vec<Hit> {
    let Some(tree) = file.embedded.first() else {
        return Vec::new();
    };
    let base = tree.range.start();
    let setup = region::blocks(file.root, file.text)
        .find(|b| b.setup)
        .map(|b| b.content - base);
    let cx = Cx {
        root: &tree.root,
        setup,
        model: OnceCell::new(),
    };
    let scan = props::scan(&cx);
    let mut hits = Vec::new();
    let declared =
        (scan.props.iter().flat_map(|d| &d.props)).chain(scan.objects.iter().flat_map(|(_, p)| p));
    for p in declared {
        for (check, on) in [
            (Check::DefaultProp, p.no_default),
            (Check::PropTypes, p.no_type),
        ] {
            if on {
                hits.push(Hit {
                    check,
                    range: js::span(&p.node) + base,
                    name: p.label.clone(),
                });
            }
        }
    }
    for m in &scan.models {
        if let Some(node) = &m.no_type {
            hits.push(Hit {
                check: Check::PropTypes,
                range: js::span(node) + base,
                name: m.name.clone(),
            });
        }
    }
    let mut dupes = Vec::new();
    for (obj, _) in &scan.objects {
        keys::options(obj, &mut dupes);
    }
    if !scan.props.is_empty() || !scan.models.is_empty() {
        keys::setup(&cx, &scan, &mut dupes);
    }
    hits.extend(dupes.into_iter().map(|(range, name)| Hit {
        check: Check::DupeKeys,
        range: range + base,
        name,
    }));
    hits
}
