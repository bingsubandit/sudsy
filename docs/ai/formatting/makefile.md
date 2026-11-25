# Makefile (`lang-makefile`)

## Parser
tree-sitter-make 1.1.1 (GNU make, forked in `grammars/tree-sitter-make`) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is the `mapping` of `crates/lang-makefile/lang.yaml`, the tree shape `grammars/makefile.grammar`. The language is a composite with no injected grammar: text no mapped node covers (function names, the parentheses of `ifeq`) is a `Word` and a `\` ending a line is whitespace. A `Rule` holds `Targets`, a `Colon`, `Prereqs` and a `Recipe` of `RecipeLine`s; an `Assign` is `Word Op Value`; `include`, `export`, `ifeq` and the like are `Directive`s, `ifeq` to `endif` a `Conditional`, `define` to `endef` a `Define` (its body one `Raw` token). References (`$(CC)`, `$@`) and quoted strings are single tokens.

Files: `Makefile`, `GNUmakefile`, `makefile`, `*.mk` and `*.mak`.

The grammar is a fork ([../../../grammars/tree-sitter-make/FORK.md](../../../grammars/tree-sitter-make/FORK.md)); 1,008 of the 1,021 files of the evidence corpus ([../design/makefile.md](../design/makefile.md)) parse, stable on a second format; the other 13 are NMake (`!IF`, `!INCLUDE`, 12 of vim) and one file that is not make syntax.

## Formatting
- One statement per line group, blank lines kept up to `max_blank_lines` (2), also after `ifeq`, before `else` and `endif`, and at the start of a `define` body; trailing blanks of a line removed, except the trailing blank of an assignment value, which is part of the value and stays.
- Assignments: one space around the operator (`A:=b`, `A+=b`, `A?=b` are spaced, `space_around_assignment`, also after `export` and in a target-specific assignment); the value is kept as written, and so are the lines after a continuation, with the blanks before a backslash collapsed to one (make reads them as one).
- Rules: `targets: prerequisites | order-only`, one space after the colon and none before it (`space_before_colon`), `&:` keeps its blank, an inline command stays after its semicolon. The lines after the first of a rule or directive are kept as written (`continued_line_indent: indent` writes one level, `indent_size`).
- Recipe commands start with exactly one tab and are kept as written, after their prefix (`@`, `-`, `+`); continued lines are kept as written. A command is Shell code (below); the Shell formatter does not run on it.
- Conditionals: the lines inside keep their indentation, comments included; `else`, `else ifeq ...` and `endif` too (`conditional_body_indent: indent` indents the body one level per nesting, not inside a recipe, where a body line is a command and starts with the tab). The condition is kept as written with its blanks collapsed.
- `define`: the body and its blanks are kept exactly, `endef` keeps its own indentation.
- A value, directive or command that ends in an odd number of `\` keeps the line it continues onto, so the next statement is not swallowed (a pair is an escaped backslash).
- Comments stay where they are. The safety check allows only blanks around values, keywords, operators and recipe prefixes to change.

The options follow mbake's (`space_around_assignment`, `space_after_colon` always on, `max_consecutive_empty_lines`, tab recipes); checkmake only lints.

<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `space_around_assignment` | bool | `true` | Write `A := b` rather than `A:=b`. |
| `space_before_colon` | bool | `false` | Write `target : prerequisite` rather than `target: prerequisite`. |
| `max_blank_lines` | int 0-10 | `2` | Largest number of blank lines kept between statements and comments. |
| `keep_comment_gap` | bool | `false` | A comment after code keeps the blanks the source has before it when they are more than one. |
| `spaced_assignments` | `all`, `plain` | `all` | The assignments `space_around_assignment` applies to: `all`; `plain`: those at the start of a line whose name is letters, digits and underscores, with no substitution reference `$(v:a=b)` on the line and no colon in the first word of a value that follows `=` at once (`A=b:c`, unless a URL, a time or a path). Any other (an indented one, one after `export` or `override`, a target-specific one, a name with `-`, `.`, `/` or a reference) keeps its blanks as written. |
| `conditional_body_indent` | `keep`, `indent` | `keep` | The lines inside a conditional (`ifeq` ... `endif`) outside a recipe: `keep` leaves their indentation as written; `indent` indents the body one level (`indent_size`, `indent_style`) per nesting. |
| `continued_line_indent` | `keep`, `indent`, `first` | `keep` | The lines after the first of a rule, directive, variable value or recipe command written over several lines: `keep` leaves their indentation as written; `indent` indents the lines of a rule or directive one level; `first` gives every line the indentation of the second (a recipe command that has a shell block keyword, and a line that starts inside quotes, keep theirs). |
<!-- /generated -->

## Presets
`makefile/checkmake`: turns on every rule that has a checkmake equivalent ([../parity/checkmake.yaml](../parity/checkmake.yaml)) at checkmake's defaults.

`makefile/mbake`: mbake's defaults (`indent_style: tab`, `continued_line_indent: first`, `spaced_assignments: plain`, `keep_comment_gap: true`, `max_blank_lines: 1`): the Sudsy side of the mbake comparison.

## Shell in recipes
Each command of a recipe is one shell invocation in make and one Shell parse here ([../architecture/adding-a-language.md](../architecture/adding-a-language.md#embedded-languages), the `embed` rows of `grammars/makefile.grammar`): from the command after its prefix characters to its last line. What the shell never sees is carved out and blanked: `$(...)`, `${...}` and `$x` references and calls, the first `$` of `$$`, and the tab that starts each continued line. The shell rules (`cd-without-exit`, the shellcheck ones, the `shell:` section of the config) run on it at their positions in the Makefile, and a `# sudsy-disable-next-line` comment of the Makefile silences them. Shell code in `$(shell ...)` and in `define` bodies is not parsed.

## Rules
Shared: suppression comments (`# sudsy-disable-next-line <rule>`), `no-empty-comment`, `comment-space`, `max-line-length`. The grammar has no `#mapping` or `#sequence` nodes, so the key rules do not apply. The language rules are listed in [../rules/builtin.md](../rules/builtin.md) and are checkmake's: `timestamp-expanded` is on by default; `phony-declared` (it names object files), `unique-targets` (a target may have several rules), `min-phony` (options `required`) and `max-body-length` (`max`) are off; the preset turns all five on. checkmake reads a rule with several targets (`a b:`) as one target named `a b`; Sudsy checks each name. A recipe ends where make's does (blank lines and comments inside it belong to it), so `max-body-length` counts more lines than checkmake in a recipe with a blank line or a column-0 comment.

## Parity
`scripts/parity.sh format mbake` ([../development/parity.md](../development/parity.md)) runs mbake 1.5.0 and `sudsy format` with the `makefile/mbake` preset on the 1,021-file corpus ([../design/makefile.md](../design/makefile.md)): the share of identical files is in the README table, `second-format` is 0. The differences are mbake's own rules and are named in `tests/parity/mbake/differences.yaml`:

<!-- generated:differences:mbake -->

- `nmake-syntax`: a file in NMake syntax (`!IF`, `!INCLUDE`, `$(VAR:a=b)` in macros, `<<` inline files), or an invalid file; GNU make cannot read it, so it is not Makefile input
<!-- /generated -->

`scripts/parity.sh checkmake` ([../development/parity.md](../development/parity.md#checkmake)) runs checkmake 0.3.2 and `sudsy lint` on the rule fixtures (`tests/rules/makefile/*/*.mk`): 10 findings, none differs but the 4 named in `tests/parity/checkmake/differences.yaml`.

Where Sudsy differs from checkmake by design:

<!-- generated:differences:checkmake -->

- `comment-before-recipe`: the tool reads a target whose recipe follows comment lines as having none; make attaches the recipe to the target
- `double-colon-rule`: a repeated `::` rule is allowed, as make does; checkmake reports it
- `empty-recipe`: a rule with an explicit empty recipe (`$(OBJS): ;`) has a body; checkmake reads it as none
- `multi-target-rule`: each target of a rule like `c d:` is checked on its own; checkmake reads one target named `c d`
- `phony-continuation`: the tool does not read the lines a `.PHONY` list continues over with a backslash; they declare targets
- `phony-line-after`: the tool reports a missing .PHONY target on the line after the last `.PHONY` line; Sudsy reports the `.PHONY` line itself
- `rule-in-define`: the tool reads the lines inside a `define` block as rules; they are the text of a macro
- `rule-in-variable-value`: the tool reads `name:VAR` lines inside a multi-line variable value as rules; they are the text of a variable
- `target-specific-variable`: a target-specific variable assignment is a variable; checkmake does not read it as one
- `variable-target`: a rule whose target is a variable that expands to file names only adds a prerequisite; the tool reports it as needing PHONY
<!-- /generated -->

## Conformance
`tests/conformance/fetch-makefile-corpus.sh` fetches the Makefiles of redis, git, lua, prometheus, kubernetes (`build/root`) and docker/cli at pinned commits; `cargo test --profile corpus -p lang-makefile --test integration -- --ignored --nocapture conformance` checks that each file parses (59 files), round-trips, formats with the same tokens (the safety check) and idempotently.
