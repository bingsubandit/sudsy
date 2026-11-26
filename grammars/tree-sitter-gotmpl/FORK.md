# Copy of tree-sitter-go-template

Upstream: ngalaiko/tree-sitter-go-template at commit 06b2fd2 (grammar 0.19.5, MIT, [LICENSE](LICENSE)), the root grammar `gotmpl` (`grammar.js` is `make_grammar('gotmpl')`; `dialects/helm` is not used). `src/` is upstream's generated parser, unchanged (ABI 15). Regenerate after editing `make_grammar.js` with `scripts/grammar-regen.sh gotmpl`.

Changes: `bindings/rust` is rewritten for the `tree-sitter-language` crate and the symbol the parser exports (`tree_sitter_gotmpl`; upstream's crate binds `tree_sitter_go_template`, which does not link and uses the tree-sitter 0.19 API). No grammar change.
