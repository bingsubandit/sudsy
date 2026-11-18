//! What the scope rules read besides the model: the file's flavors, grammar tags, fields and
//! own tokens of nodes. Kinds are never named here: a language says what a kind is by its tags
//! (`#function`, `#loop`, `#export`, `#signature`, ...; docs/rules/semantics.md#scope-rules).

use super::ScopeLang;
use crate::FileContext;
use crate::tree::significant_range;
use core_semantics::scopes::{Decl, Flags, Model, Reference, Symbol};
use core_syntax::roles::has_own_token;
use core_syntax::{SyntaxNode, TextRange};

pub(super) struct Cx<'a> {
    pub m: &'a Model,
    pub file: &'a FileContext<'a>,
    pub lang: &'a ScopeLang,
    /// The file declares types (`.ts`): declaration merging, ambient names, type-only uses.
    pub typed: bool,
    /// Every declaration of the file is ambient (`.d.ts`).
    pub definition: bool,
}

impl<'a> Cx<'a> {
    pub fn new(m: &'a Model, file: &'a FileContext<'a>, lang: &'a ScopeLang) -> Cx<'a> {
        Cx {
            m,
            file,
            lang,
            typed: lang.flavor("typed", file.path),
            definition: lang.flavor("definition", file.path),
        }
    }

    pub fn tag(&self, n: &SyntaxNode, tag: &str) -> bool {
        self.file.grammar.has_tag(n.kind(), tag)
    }

    pub fn is_function(&self, n: &SyntaxNode) -> bool {
        self.tag(n, "function")
    }

    /// The child node in the field `label` of `n`.
    pub fn slot(&self, n: &SyntaxNode, label: &str) -> Option<SyntaxNode> {
        self.file.grammar.slot(n, label)?.into_node()
    }

    pub fn has_own(&self, n: &SyntaxNode, text: &str) -> bool {
        has_own_token(n, text)
    }

    /// The span of `n` without the trivia around it.
    pub fn range(&self, n: &SyntaxNode) -> TextRange {
        significant_range(n, self.file.grammar)
    }

    pub fn root(&self) -> &SyntaxNode {
        self.file.root
    }

    pub fn node_of(&self, r: &Reference) -> SyntaxNode {
        r.node(self.file.root)
    }

    pub fn decl_node(&self, d: &Decl) -> SyntaxNode {
        d.node(self.file.root)
    }

    pub fn binding(&self, d: &Decl) -> SyntaxNode {
        d.binding(self.file.root)
    }

    /// Does the declaration write the name it declares (`let a = 1`, a loop variable, a default)?
    pub fn writes_declaration(&self, d: &Decl) -> bool {
        let n = self.decl_node(d);
        if d.flags.intersects(Flags::PARAMETER) {
            return self.has_default(&self.binding(d));
        }
        if !d.flags.intersects(Flags::VAR | Flags::LET | Flags::CONST) {
            return false;
        }
        self.slot(&n, "init").is_some() || self.in_foreach_head(&n)
    }

    /// Is the declarator `decl` part of the head of a `for ... in/of`?
    pub fn in_foreach_head(&self, decl: &SyntaxNode) -> bool {
        decl.parent()
            .and_then(|d| d.parent())
            .is_some_and(|s| self.is_foreach(&s))
    }

    /// A loop over an `iterable`.
    pub fn is_foreach(&self, n: &SyntaxNode) -> bool {
        self.tag(n, "loop") && self.file.grammar.has_field(n.kind(), "iterable")
    }

    /// Does the binding take a default (`a = 1`, `{ a = 1 }`), which writes it?
    pub fn has_default(&self, binding: &SyntaxNode) -> bool {
        for a in binding.ancestors().skip(1) {
            if self.tag(&a, "default-value") {
                return true;
            }
            if self.tag(&a, "parameter") {
                return self.param_init(&a).is_some();
            }
            if self.tag(&a, "parameter-list") {
                return false;
            }
        }
        false
    }

    /// The initializer of a parameter (`a = 1`): the node after its `=`.
    pub fn param_init(&self, param: &SyntaxNode) -> Option<SyntaxNode> {
        let eq = param
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.text() == "=")?;
        param
            .children()
            .find(|c| c.text_range().start() >= eq.text_range().end())
    }

    /// The parameter list a binding belongs to, when it is one of a function.
    pub fn param_list(&self, binding: &SyntaxNode) -> Option<SyntaxNode> {
        for a in binding.ancestors().skip(1) {
            if self.tag(&a, "parameter-list") {
                return Some(a);
            }
            if self.is_function(&a) {
                return None;
            }
        }
        None
    }

    /// Is the parameter `binding` one of a function type, a signature or a function declaration without a body?
    pub fn in_signature(&self, binding: &SyntaxNode) -> bool {
        let Some(owner) = binding
            .ancestors()
            .find(|a| self.tag(a, "parameter-list"))
            .and_then(|l| l.parent())
        else {
            return false;
        };
        self.tag(&owner, "signature")
            || (self.is_function(&owner)
                && self.slot(&owner, "body").is_none()
                && self.slot(&owner, "expression_body").is_none())
    }

    /// Is the declaration an import of a type only (`import type`, `{ type A }`)?
    pub fn is_type_import(&self, d: &Decl) -> bool {
        d.flags.contains(Flags::IMPORT) && {
            let n = self.decl_node(d);
            self.has_own(&n, "type")
                || n.parent()
                    .is_some_and(|p| self.tag(&p, "import") && self.has_own(&p, "type"))
        }
    }

    /// Does the symbol name a value (a variable, function, class, enum, value import)?
    pub fn is_value(&self, s: &Symbol) -> bool {
        s.flags.contains(Flags::VALUE) && !s.decls.iter().all(|d| self.is_type_import(d))
    }

    /// The statement a declaration is written in: its declarator's declaration, or itself.
    pub fn statement_of(&self, d: &Decl) -> Option<SyntaxNode> {
        let n = self.decl_node(d);
        if d.flags.intersects(Flags::VAR | Flags::LET | Flags::CONST) {
            n.parent()
        } else {
            Some(n)
        }
    }
}
