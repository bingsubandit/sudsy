// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const BINARY_OPERATOR_LINE: core_config::OptId = core_config::opt::BINARY_OPERATOR_LINE;
#[allow(dead_code)]
pub const SPACE_REDIRECTS: core_config::OptId = core_config::opt::SPACE_REDIRECTS;
#[allow(dead_code)]
pub const INDENT_CASE_ARMS: core_config::OptId = core_config::opt::INDENT_CASE_ARMS;
#[allow(dead_code)]
pub const KEEP_FINAL_NEWLINE: core_config::OptId = core_config::opt::KEEP_FINAL_NEWLINE;
#[allow(dead_code)]
pub const COMMENT_SPACE: core_config::OptId = core_config::opt::COMMENT_SPACE;
#[allow(dead_code)]
pub const KEY_VALUE_PAIRS: core_config::OptId = core_config::opt::KEY_VALUE_PAIRS;
#[allow(dead_code)]
pub const JSON_COPY_AS_WORDS: core_config::OptId = core_config::opt::JSON_COPY_AS_WORDS;
#[allow(dead_code)]
pub fn space_redirects(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_REDIRECTS, true)
}
#[allow(dead_code)]
pub fn indent_case_arms(o: &core_config::ResolvedOptions) -> bool {
    o.flag(INDENT_CASE_ARMS, true)
}
#[allow(dead_code)]
pub fn keep_final_newline(o: &core_config::ResolvedOptions) -> bool {
    o.flag(KEEP_FINAL_NEWLINE, false)
}
#[allow(dead_code)]
pub fn comment_space(o: &core_config::ResolvedOptions) -> bool {
    o.flag(COMMENT_SPACE, false)
}
#[allow(dead_code)]
pub fn key_value_pairs(o: &core_config::ResolvedOptions) -> bool {
    o.flag(KEY_VALUE_PAIRS, false)
}
#[allow(dead_code)]
pub fn json_copy_as_words(o: &core_config::ResolvedOptions) -> bool {
    o.flag(JSON_COPY_AS_WORDS, false)
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(4),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between instructions and comments."),
        core_config::shared::binary_operator_line("start"),
        core_config::OptionDef::boolean(core_config::opt::SPACE_REDIRECTS, true, "A blank after the operator of a redirect that opens a file in the shell code of `RUN` (`> file`)."),
        core_config::OptionDef::boolean(core_config::opt::INDENT_CASE_ARMS, true, "The arms of a `case` in the shell code of `RUN` one level inside it."),
        core_config::OptionDef::boolean(core_config::opt::KEEP_FINAL_NEWLINE, false, "A file that does not end with a line break stays without one (the final newline is not added)."),
        core_config::OptionDef::boolean(core_config::opt::COMMENT_SPACE, false, "A line comment whose `#` run is followed by a letter or digit gets a blank after it (`#x` as `# x`); a parser directive before the first instruction stays."),
        core_config::OptionDef::boolean(core_config::opt::KEY_VALUE_PAIRS, false, "`ENV` and `LABEL` in the legacy form `key value` as `key=value` (a value with blanks in double quotes), where both read as the same value: the value has no quote and no backslash, or is one word (a backslash stays as written), or is one quoted string."),
        core_config::OptionDef::boolean(core_config::opt::JSON_COPY_AS_WORDS, false, "The JSON array of `COPY` and `ADD` as plain arguments (`COPY [\"a\", \"b\"]` as `COPY a b`) where every element is a plain path word (letters, digits and `_./:@%+,=~*?^-`, not starting with `-`) and the array is on one line."),
        ])
        .with_presets(vec![
        core_config::Preset::new("dockerfile/hadolint", include_str!("../presets/hadolint.yaml")),
        core_config::Preset::new("dockerfile/dockerfmt", include_str!("../presets/dockerfmt.yaml")),
        ])
    })
}
