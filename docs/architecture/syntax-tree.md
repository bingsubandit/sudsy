# Syntax tree, grammars, wire encoding

## Tree
- `rowan` green/red tree (vendored in `third_party/rowan` with a 32-bit `SyntaxKind`; upstream is 16-bit); kinds are `u32`: 8 bits language (0 core, 1 JSON, 2 C#, 3 YAML, ...), 1 token bit, 23 bits index.
- Lossless: printing the tree reproduces the source byte for byte; every conformance test checks it.
- Trivia (whitespace, newlines, comments) are tokens. C# doc comments and directives are structured trivia nodes (tag `#trivia`); disabled `#if` text is one verbatim token.
- `ERROR` nodes hold what could not be parsed; every token stays in the tree. Formatter and rules skip subtrees with errors; files with syntax errors are not formatted.
- Code of another language is parsed beside the host's tree, not inside it ([adding-a-language.md](adding-a-language.md#embedded-languages)).
- A zero-width EOF token takes the leading trivia of the file's tail.

## Trivia attachment (every language)
Trailing trivia: after a token up to and including the end of its line. Leading: everything else before a token. A run between two tokens is split after the first trivia token ending in a line break (`\n`, `\r`, NEL, LS, PS); a run without one is all trailing. `trivia::check_attachment` verifies it (also inside structured trivia).

## Grammars (`grammars/<lang>.grammar`)
- Line-based, ungrammar-like: tokens, nodes with named child fields (`name:Type`, `?`, `*`), comment syntax, `suppress` markers.
- Field types are a kind name or a `#tag`. Tags (`#block`, `#delimited-list`, `#key-value`, `#separator`, `#string`, `#comment`, `#binary-chain`, `#member-chain`, `#restyle`, ...) drive shared shapes, rules and patterns; they double as union types in generated wrappers.
- Bodies match children in order (`core_syntax::ast::assign`); `ShapeChecker` verifies every child lands in a slot (`#opaque` nodes declare no shape and are skipped; `violations` lists every failing node, `provider_api::format_testing::check_corpus_shapes` runs it over a corpus). Generated accessors use the same routine, and so does `Grammar::slot(node, label)`, the generic accessor for a labelled item (shared rules read an entry's `key` and `value` with it; bodies are resolved to kind lists once when the grammar loads).
- Tags for shared rules: `#mapping` marks a container of key/value entries (the entries are `#key-value` nodes whose key is the item labelled `key`: JSON `Object`; YAML `BlockMapping`, `FlowMapping`), `#sequence` a list of values (JSON `Array`; YAML `BlockSequence`, `FlowSequence`), `#sections` a file container of sections whose first child, a `#section` header node, has the name token in its `name` slot (INI, EditorConfig `Document`, `Section`). A rule subscribes by tag (`Rule::tags`) and runs for every kind carrying it, in every language that tags its grammar. `Grammar::nodes_with_tag` answers from a table built at load.
- Roles: a node kind says what it is for cross-language code (layout kits, scope binder, shared rules) with a role tag from the vocabulary below, or `#opaque` when none fits. A kind may carry several roles (a method is a `#declaration` and a `#function`). A role names the fields shared code reads with `Grammar::slot(node, field)`; `field?` is optional, and a required field must be declared in the body, as the item's label or as an alias `label=field:Type` (C# `when_true=then:#expression`; the generated accessor keeps `label`). A grammar opts in with a `roles` line; then `cargo xtask codegen` refuses a node kind with neither a role nor `#opaque` (or with both), and a role whose required fields the body lacks. Tokens are exempt. The table is `core_syntax::roles::ROLES`; adapter-backed grammars (oxc, ruff, ...) opt in the same way and their adapter fills the nodes as the body says.
- Decisions every grammar follows:
  - A kind has a role when its body can declare every required field as items of its own; otherwise it is `#opaque`. So `#conditional` is the ternary only and an `if` statement (optional `else`) is `#opaque`; `new` is `#call` (`callee` is the constructed type or expression: JS `NewExpression`, Java `ObjectCreationExpression`; C#'s `ObjectCreationExpression` stays opaque); a statement or expression with no field to read (`return`, `throw`, JSX, TypeScript types) is `#opaque`. A kind with a role needs its whole body, as the shape check reads it; an `#opaque` kind needs none.
  - One field can be several items (`label=field`; labels are unique, fields are not). `Grammar::slot` returns the first child assigned to any of them, in the order of the body. This is the union: a postfix `operator` after the operand and a prefix one before it (`prefix=operator:... operand suffix=operator:...`), and a Java operand that is a node (`left:#expression?`) or a bare token (`left_token=left:#operand?`; tags are uniform, so one item cannot take both). When a required token of the same kind follows, the token item goes first: Java `#type-token? #type? name:Identifier` (a node return type must not leave `name` to a token item).
  - A field that is a list in the language but has no list node in the parser's tree is a repeated item (`arguments:#expression*` of a JS call, whose `(`, commas and `)` are children of the call); `slot` returns the first, a layout walks the children between the delimiters.
  - Union tags a body names: `#expression`, `#statement`, `#member` (class, interface and enum members), `#pattern`, `#parameter` (nodes); `#identifier` (Identifier and Keyword: oxc has reserved words as names, `as const`, `yield`), `#operator` (every token that can be an operator: JS `Operator`, `Keyword` for `in` and `typeof`, `LAngle`, `RAngle`; Java `Operator`, `Lt`, `Gt`, `Assign`), `#operand` and `#type-token` (Java tokens that stand for an expression or a type) are tokens.
  - JS: `IdentifierReference` is `#reference` and `BindingIdentifier` `#declaration` (the binding site); the named constructs (function, class, method, property, variable declarator, enum, interface) are `#declaration` too, with the name as a child `BindingIdentifier` or key (`name:#expression`), not a token. `#scope` kinds: program, function, arrow, block, class, static block, `for` statements, `catch`, `switch`, module block, interface. Parenthesized assignment targets and `for` heads keep their `(` `)` as children of the statement or expression. The adapter sorts oxc's nodes by start so that a function's `this` parameter and decorators before `export` are in source order.
  - Java: identifiers and literals are bare tokens in expressions (rezel has no node for them), so Java has no `#reference` or `#literal` node yet; roles sit on the `*Core` nodes of type declarations (`ClassDeclarationCore` holds the name and the body, `ClassDeclaration` is a wrapper with the modifiers) and on `TypeOrStaticImportDeclaration`; `MethodInvocation` is `#call` with `callee` the method name and `object` the receiver (a member access and a call in one node).
  - Parts the parser does not emit are optional items, and no adapter wraps them: the implicit receiver of a Java `f()` (`object`), a JS `new Foo` without `(`. Node kinds with a role / `#opaque`: JS 44 / 147, Java 37 / 112.

<!-- generated:roles -->
| Role | Fields | Meaning |
|---|---|---|
| `#call` | `callee` `arguments` | calls a function or method |
| `#member-access` | `member` `object?` | `object.member`; the object is absent in a binding (`.member`) |
| `#index` | `arguments` `object?` | `object[arguments]`; the object is absent in a binding |
| `#binary` | `left` `operator` `right` | two operands and an operator, any precedence |
| `#unary` | `operand` `operator?` | prefix, postfix or keyword operator on one operand |
| `#assignment` | `left` `operator` `right` | stores a value, plain or compound |
| `#conditional` | `condition` `then` `else` | ternary expression |
| `#function` | `parameters` `body?` `expression_body?` | method, lambda or local function |
| `#parameter-list` | `parameters` | the parameters of a function with their brackets |
| `#block` | `items` | braced sequence of statements or members |
| `#declaration` | `name?` | introduces a name (member, accessor, namespace) |
| `#type-declaration` | `members` `name?` | class, struct, interface, enum |
| `#import` | `path` | brings the names of another file or namespace into scope |
| `#literal` | `value` | a constant written in the source |
| `#scope` | - | names declared inside are not visible outside |
| `#reference` | `name` | uses a name |
<!-- /generated -->

- `grammars/csharp.grammar` follows Roslyn's field lists (321 node kinds, 252 token kinds); hand-maintained. Types are also expressions (every `#type` kind carries `#expression`).
- The typed wrappers (`generated` in each language crate) and the grammar registry (`core_syntax::grammars`) are generated by build scripts (`build/lang-crate.rs`, `core-syntax/build.rs`) from `grammars/*.grammar` into `OUT_DIR`, so a grammar file is the whole registration. `cargo xtask codegen` writes the worker's kind table (`workers/csharp/Sudsy.Worker/Generated/CsharpGrammar.g.cs`), `schemas/sudsy.schema.json`, the docs' tables and the editor files; `xtask/full/tests/codegen_is_fresh.rs` fails on stale output.
- `language <name> <id> ["Display"]`: the display name (`C#`) is what generated language lists print; it defaults to the name.

## Wire encoding (`core-syntax/src/wire.rs`)
Per file, never per node; token text is not sent (the receiver has the source).
- `version: u32`, `kinds`: `bin` of little-endian u32 (version 2), `values`: `bin` of LEB128 varints (token: UTF-8 width; node: child count), inside the MessagePack message.
- Node widths are derived from children. Whether an element is a token is a bit of its kind.
- Why flat arrays in `bin`: smallest encode+decode overhead (4.8% of Roslyn parse on CoreLib) and no second container format.
- The real cost is not the encoding but Roslyn's red-node materialization during the mapping walk (see [numbers](../performance/numbers.md)).
- Decoding rejects trees nested deeper than `MAX_DEPTH` (2048 levels) with `WireError::TooDeep`: rowan drops a green tree recursively, once per level, and a debug build on a 2 MB stack survives about 3000 levels. The file then fails to parse instead of overflowing the stack.
