# Design: Ruby, the next steps

Goal: formatter and linter for `.rb`, `.rake`, `.gemspec`, `.ru` files (Ruby 3.3 to 4.0 syntax). The parser, the tree and the verbatim formatter are in [../formatting/ruby.md](../formatting/ruby.md); this note holds what is still open and is deleted when step 3 lands.

## Reference tools
- Format: RuboCop's Layout cops (`rubocop -a --only Layout`, the style most teams run), Standard (RuboCop with fixed config), syntax_tree (`stree`, a prettier-like formatter, closest to a pure formatter), rufo.
- Lint: RuboCop Lint cops (and Style/Naming where file-local).
- No system Ruby: the parity runner needs a Ruby. Options to check in the parity step: a prebuilt portable Ruby (ruby/ruby-builder or traveling-ruby tarballs as a `release` asset) with the gems installed by `gem install --install-dir`. Not tried.

## Steps
| Step | Builds |
|---|---|
| 2 | Formatter: RuboCop Layout style on the role-driven kit (after L1), parity runner against `rubocop` / `stree`, style options |
| 3 | Lint: RuboCop Lint cops as pattern rules and families on the roles (in: 95.6% of RuboCop's default Lint findings, [../formatting/ruby.md](../formatting/ruby.md#lint)) |
