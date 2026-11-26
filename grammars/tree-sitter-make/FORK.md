# Fork of tree-sitter-make

Upstream: crates.io `tree-sitter-make` 1.1.1 (tree-sitter-grammars/tree-sitter-make, MIT, [LICENSE](LICENSE)). Regenerate `src/` after editing `grammar.js` with `scripts/grammar-regen.sh make`.

`bindings/rust` is trimmed to the language constant (no queries or doctest) so the workspace's fmt and clippy pass; `tree-sitter generate` does not touch it.

Changes to `grammar.js` (valid makefiles the upstream grammar rejected):
- `function_call`: the blank after the function name belongs to the name token, so a variable named like a function (`$(dir)`, `$(info)`) is a reference.
- `arguments`: an argument may be empty (`$(if $(a),-b,)`).
- `text`: balanced parentheses (`_paren_group`) in a value, an argument or a `$(shell ...)` (`SOURCES = ( \ ... )`, `$(warning goal(s))`).
- `$$` no longer starts a reference or call (`variable_reference`, `substitution_reference`, `automatic_variable`, `function_call`, `shell_function`): it is the escape of a `$`, so `$$(cmd args)` in a recipe is shell text.
- `substitution_reference`: an automatic variable as the text (`$(@:%.o=%.c)`).
- Names: a variable or target name may be a reference first (`OBJS-$(X) += a`, `$(a) = 1`), may start with `!`, and `CHARSET` stops at `[` and `]`.
- Values: a stray `)` and `case ... in 0) ... esac` text are plain value text; `$()` and `$([TO])` are references; `#` right after a reference in a recipe is shell text.
- Statements: a line that is only a reference (`$(recurse)`) or a reference with stray `)` (`$(error x))`) is a `function_call`; a lone `@` recipe line is a recipe.
- `define`: a name with a reference (`define A/$(1)`), and nested `define`/`endef` pairs are counted as body text.
- Target-specific assignments take `export`, `override` and `private` after the colon (`x: export A := b`).
- Conditionals: an argument may be a bare word with `;` or `:` (`ifeq ($(a), b;)`); `else` and `elsif` accept a trailing blank.
- `substitution_reference`: pattern and replacement are optional (`$(B:=.o)`, `$(B:/=)`) and may hold braces (`$(c:%=a{b})`, `$(B:%=-Wl,%)`).
- `variable_reference`: `$` and a backslash-newline is the empty variable that joins two lines (`x|$\` newline `y`).
- Words: a word does not end in `+`, `?` or `!` when an assignment operator follows (`A+=b` is the word `A` and `+=`); the run is a `_word_tail`. The parser has 1,570 states (the upstream 1,383), kept small by hidden rules for the substitution body (`_subst_body`, `_subarg`) and by `ifeq "a" "b"` reading plain primaries (the bare-word form with `;`, `:`, `=` is only in the parenthesized form).
