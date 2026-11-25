# Fork of tree-sitter-containerfile

Upstream: crates.io `tree-sitter-containerfile` 0.9.2 (wharflab/tree-sitter-containerfile at 1df9124b22a72dde8683aab6f762ff4857acfeab, MIT, [LICENSE](LICENSE)). Regenerate `src/parser.c` after editing `grammar.js` with `scripts/grammar-regen.sh containerfile`; `src/scanner.c` is upstream's but for the `#` rule below.

`bindings/rust` is trimmed to the language constant (no queries or doctest, no wasm branch) so the workspace's fmt and clippy pass; `tree-sitter generate` does not touch it. The upstream queries, README and Cargo.lock are not copied.

Changes to `grammar.js` (valid Dockerfiles the upstream grammar rejected):
- `shell_command`: a fragment after a line continuation may start with `[` (`_shell_continued_fragment`), so a script line such as `[ -s x ]; \` is shell text. Only the first fragment of a command can be a JSON array. 81 of the 83 corpus files for which tree-sitter reported an error (docker-library python, postgres, golang) are now clean.
- `_shell_single_quoted_fragment` and the single-quoted regexes of shell text: a backslash before the closing quote is literal (`RUN x 'C:\dir\'; \`).
- `user_instruction`: the user may be a quoted string (`USER "$UID"`).
- `arg_pair`: the name is any word without `=`, quotes, `$` and `#`, so a template default (`ARG X={{ v }}`) parses.
- `HEALTHCHECK CMD` takes `_json_or_shell_command`, so a test command that starts with `[` (`[ "$(curl x)" -eq "200" ]`) is shell text.

`src/scanner.c`: a `#` is a comment at column 0 or after a blank, not after other text (`ADD ${A}#${B} /`); `expansion_operator` takes `#`, `##`, `%`, `%%` (`ARG X=${V#linux/amd64}`); a `[` that starts the text is tested for an invalid JSON array on its own (the scan moves the lexer, so no other token is tried after it).
