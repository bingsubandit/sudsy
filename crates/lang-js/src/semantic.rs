//! The scope model of a file, for the rules that need to know what a name binds to: the shared
//! binder of `core_semantics::scopes` run over the tree with JavaScript's table (which kinds hold
//! names, refer to names or assign). Built on the first scope rule that runs on a file and shared
//! with the others, so a run with syntax rules only never pays for it.

use crate::adapter::source_type;
use core_rules::FileContext;
use core_rules::checks::FileCache;
use core_rules::tree::significant_range;
use crate::generated::kind as k;
use core_semantics::scopes::{
    Decl, Declares, Flags, Flow, Model, Place, Row, Sel, Symbol, SymbolId, Table, bind,
};
use core_syntax::{Grammar, SyntaxNode, SyntaxNodeExt, TextRange, grammars};
use std::cell::OnceCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

const V: Flags = Flags::VALUE;
const T: Flags = Flags::TYPE;

const fn declares(flags: Flags, place: Place) -> Declares {
    Declares {
        flags,
        place,
        ..Declares::NONE
    }
}

const fn var_scope(kind: &'static str) -> Row {
    Row {
        kind,
        var_scope: true,
        ..Row::NONE
    }
}

const fn names(kind: &'static str, flags: Flags, place: Place) -> Row {
    Row {
        kind,
        declares: Some(declares(flags, place)),
        ..Row::NONE
    }
}

const fn flows(kind: &'static str, children: &'static [(Sel, Flow)]) -> Row {
    Row {
        kind,
        children,
        ..Row::NONE
    }
}

const IMPORTED: Flags = V.union(T).union(Flags::IMPORT);

/// What JavaScript and TypeScript say to the binder (docs/formatting/js.md#scope-layer).
const ROWS: &[Row] = &[
    var_scope("Program"),
    var_scope("ArrowFunctionExpression"),
    var_scope("StaticBlock"),
    var_scope("TSModuleBlock"),
    Row {
        kind: "Function",
        var_scope: true,
        declares: Some(Declares {
            flags: V.union(Flags::FUNCTION),
            place: Place::Parent,
            expression: Some((Flags::EXPRESSION_NAME, Place::Scope)),
            ..Declares::NONE
        }),
        ..Row::NONE
    },
    Row {
        kind: "Class",
        declares: Some(Declares {
            flags: V.union(T).union(Flags::CLASS),
            place: Place::Parent,
            expression: Some((Flags::EXPRESSION_NAME, Place::Scope)),
            ..Declares::NONE
        }),
        ..Row::NONE
    },
    Row {
        kind: "VariableDeclarator",
        declares: Some(Declares {
            flags: V.union(Flags::VAR),
            place: Place::Var,
            keywords: &[
                ("var", V.union(Flags::VAR), Place::Var),
                ("let", V.union(Flags::LET), Place::Scope),
                ("const", V.union(Flags::CONST), Place::Scope),
                ("using", V.union(Flags::CONST), Place::Scope),
            ],
            keyword_on_parent: true,
            ..Declares::NONE
        }),
        ..Row::NONE
    },
    names("FormalParameter", V.union(Flags::PARAMETER), Place::Scope),
    names("FormalParameterRest", V.union(Flags::PARAMETER), Place::Scope),
    names("CatchParameter", V.union(Flags::CATCH), Place::Scope),
    names("TSEnumDeclaration", V.union(T).union(Flags::ENUM), Place::Parent),
    Row {
        kind: "TSEnumMember",
        declares: Some(Declares {
            flags: V.union(Flags::ENUM_MEMBER),
            name_child: true,
            ..Declares::NONE
        }),
        ..Row::NONE
    },
    names("TSInterfaceDeclaration", T.union(Flags::INTERFACE), Place::Parent),
    names("TSTypeAliasDeclaration", T.union(Flags::TYPE_ALIAS), Place::Parent),
    names("TSNamespaceDeclaration", V.union(T).union(Flags::NAMESPACE), Place::Scope),
    Row {
        kind: "TSTypeParameter",
        declares: Some(Declares {
            flags: T.union(Flags::TYPE_PARAMETER),
            parent_places: &[("TSInferType", Place::Enclosing("TSConditionalType"))],
            ..Declares::NONE
        }),
        ..Row::NONE
    },
    names("TSMappedType", T.union(Flags::TYPE_PARAMETER), Place::Scope),
    names("ImportSpecifier", IMPORTED, Place::Scope),
    names("ImportDefaultSpecifier", IMPORTED, Place::Scope),
    names("ImportNamespaceSpecifier", IMPORTED, Place::Scope),
    names("TSImportEqualsDeclaration", IMPORTED, Place::Scope),
    Row {
        kind: "BindingIdentifier",
        binding: true,
        ..Row::NONE
    },
    Row {
        kind: "IdentifierReference",
        reference: true,
        ..Row::NONE
    },
    flows("AssignmentExpression", &[(Sel::BeforeOp, Flow::Assign("="))]),
    flows("UpdateExpression", &[(Sel::All, Flow::ReadWrite)]),
    flows("ForInStatement", &[(Sel::Field("left"), Flow::Write)]),
    flows("ForOfStatement", &[(Sel::Field("left"), Flow::Write)]),
    flows("ArrayAssignmentTarget", &[(Sel::All, Flow::Through)]),
    flows("ObjectAssignmentTarget", &[(Sel::All, Flow::Through)]),
    flows("AssignmentTargetRest", &[(Sel::All, Flow::Through)]),
    flows("AssignmentTargetWithDefault", &[(Sel::First, Flow::Through)]),
    flows("AssignmentTargetPropertyIdentifier", &[(Sel::First, Flow::Through)]),
    flows("AssignmentTargetPropertyProperty", &[(Sel::AfterText(":"), Flow::Through)]),
    flows("ParenthesizedExpression", &[(Sel::All, Flow::Through)]),
    flows("TSNonNullExpression", &[(Sel::All, Flow::Through)]),
    flows("TSAsExpression", &[(Sel::First, Flow::Through)]),
    flows("TSSatisfiesExpression", &[(Sel::First, Flow::Through)]),
    flows("TSTypeReference", &[(Sel::First, Flow::Type)]),
    flows("TSClassImplements", &[(Sel::First, Flow::Type)]),
    flows("TSInterfaceHeritage", &[(Sel::First, Flow::Type)]),
    flows("TSTypeQuery", &[(Sel::All, Flow::Query)]),
    flows("TSConditionalType", &[(Sel::AfterText(":"), Flow::Outside)]),
    flows("ExportNamedDeclaration", &[(Sel::WithToken("type"), Flow::Type)]),
    flows(
        "ExportSpecifier",
        &[(Sel::WithToken("type"), Flow::Type), (Sel::First, Flow::ThroughType)],
    ),
    flows("TSQualifiedName", &[(Sel::First, Flow::ThroughType)]),
    flows("StaticMemberExpression", &[(Sel::First, Flow::ThroughType)]),
];

/// The node kinds besides blocks whose function and class children are declarations.
const HOSTS: &[&str] = &[
    "ExportDeclaration",
    "ExportNamedDeclaration",
    "ExportDefaultDeclaration",
    "SwitchCase",
    "LabeledStatement",
    "IfStatement",
    "WhileStatement",
    "DoWhileStatement",
    "ForStatement",
    "ForInStatement",
    "ForOfStatement",
    "WithStatement",
];

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| Table::new(grammars::JS.get(), ROWS, HOSTS, &["Identifier"]))
}

/// What a scope rule reads: the model and the file's kind.
pub struct Cx {
    pub s: Model,
    pub root: SyntaxNode,
    pub g: &'static Grammar,
    /// `.cjs`: the top scope is a function scope with `require`, `module` and `exports`.
    pub commonjs: bool,
    pub ts: bool,
    /// `.d.ts`, `.d.cts`, `.d.mts`: every declaration is ambient.
    pub definition: bool,
    jsx: OnceCell<bool>,
    by_binding: OnceCell<HashMap<u32, SymbolId>>,
}

impl Cx {
    /// The child node in the body item `label` (a field of the grammar).
    pub fn slot(&self, n: &SyntaxNode, label: &str) -> Option<SyntaxNode> {
        match self.g.slot(n, label)? {
            core_syntax::SyntaxElement::Node(n) => Some(n),
            core_syntax::SyntaxElement::Token(_) => None,
        }
    }

    /// Does `n` have an own token (not trivia) with this text (`static`, `set`, `type`)?
    pub fn has_own(&self, n: &SyntaxNode, text: &str) -> bool {
        n.tokens().any(|t| t.text() == text && !self.g.is_trivia(t.kind()))
    }

    /// The span of `n` without the trivia around it.
    pub fn range(&self, n: &SyntaxNode) -> TextRange {
        significant_range(n, self.g)
    }

    /// Does the file have a JSX element? typescript-eslint reads that as a use of `React`.
    pub fn has_jsx(&self) -> bool {
        *self.jsx.get_or_init(|| {
            let kind = self.g.kind_by_name("JSXOpeningElement");
            self.root.descendants().any(|n| Some(n.kind()) == kind)
        })
    }

    /// Is the import `name` the JSX pragma that the elements of the file use (TypeScript files only)?
    pub fn jsx_pragma(&self, name: &str) -> bool {
        self.ts && name == "React" && self.has_jsx()
    }

    /// The symbol a binding node declares.
    pub fn symbol_of(&self, binding: &SyntaxNode) -> Option<SymbolId> {
        let map = self.by_binding.get_or_init(|| {
            let mut map = HashMap::new();
            for id in self.s.symbol_ids() {
                for d in &self.s.symbol(id).decls {
                    map.insert(u32::from(d.name.start()), id);
                }
            }
            map
        });
        map.get(&u32::from(self.range(binding).start())).copied()
    }

    /// Is the declaration an import of a type only (`import type`, `{ type A }`)?
    pub fn is_type_import(&self, d: &Decl) -> bool {
        d.flags.contains(Flags::IMPORT)
            && (self.has_own(&d.node, "type")
                || d.node
                    .parent()
                    .is_some_and(|p| p.kind() == k::IMPORT_DECLARATION && self.has_own(&p, "type")))
    }

    /// Does the symbol name a value (a variable, function, class, enum, value import)?
    pub fn is_value(&self, s: &Symbol) -> bool {
        s.flags.contains(Flags::VALUE) && !s.decls.iter().all(|d| self.is_type_import(d))
    }

    /// The name of a kind.
    pub fn kind_name(&self, n: &SyntaxNode) -> &str {
        self.g.name(n.kind())
    }
}

thread_local! {
    static CACHE: FileCache<Cx> = const { FileCache::new() };
}

/// The model of the file, built once and shared by the rules that run on it; `None` when the
/// parse failed (the parser's diagnostic stands, a tree of one unknown token binds nothing).
pub fn model(file: &FileContext<'_>) -> Option<Rc<Cx>> {
    let cx = CACHE.with(|cache| {
        cache.get(file, || {
            let name = file.path.to_string_lossy();
            Cx {
                s: bind(file.root, file.grammar, table()),
                root: file.root.clone(),
                g: grammars::JS.get(),
                commonjs: name.ends_with(".cjs"),
                ts: source_type(&name).is_typescript(),
                definition: [".d.ts", ".d.cts", ".d.mts"]
                    .iter()
                    .any(|e| name.ends_with(e)),
                jsx: OnceCell::new(),
                by_binding: OnceCell::new(),
            }
        })
    });
    let fatal = file
        .root
        .first_token()
        .is_some_and(|t| t.kind() == k::UNKNOWN);
    (!fatal).then_some(cx)
}

/// `cargo test --release -p lang-js --lib -- --ignored model_cost --nocapture`: what binding costs
/// on the files of `bench/inputs/js` (best of 50 per file), next to the parse of the same files.
#[cfg(test)]
mod cost {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore = "prints timings"]
    fn model_cost() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../bench/inputs/js");
        let (mut bytes, mut parse, mut bound) = (0, 0.0, 0.0);
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            let st = source_type(&path.to_string_lossy());
            let (mut p, mut b) = (f64::MAX, f64::MAX);
            for _ in 0..50 {
                let start = Instant::now();
                let tree = crate::adapter::parse(&text, st).tree;
                let parsed = Instant::now();
                let model = bind(&tree, grammars::JS.get(), table());
                p = p.min((parsed - start).as_secs_f64());
                b = b.min(parsed.elapsed().as_secs_f64());
                drop(model);
            }
            bytes += text.len();
            parse += p;
            bound += b;
        }
        println!("{bytes} bytes: parse {:.2} ms, bind {:.2} ms", parse * 1e3, bound * 1e3);
    }
}
