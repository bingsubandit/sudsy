// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const EMBEDDED_LANGUAGE_FORMATTING: core_config::OptId = core_config::opt::EMBEDDED_LANGUAGE_FORMATTING;
#[allow(dead_code)]
pub const WHITESPACE_SENSITIVITY: core_config::OptId = core_config::opt::WHITESPACE_SENSITIVITY;
#[allow(dead_code)]
pub const BRACKET_SAME_LINE: core_config::OptId = core_config::opt::BRACKET_SAME_LINE;
#[allow(dead_code)]
pub const SINGLE_ATTRIBUTE_PER_LINE: core_config::OptId = core_config::opt::SINGLE_ATTRIBUTE_PER_LINE;
#[allow(dead_code)]
pub const MAX_ATTRIBUTES_PER_LINE: core_config::OptId = core_config::opt::MAX_ATTRIBUTES_PER_LINE;
#[allow(dead_code)]
pub const SELF_CLOSING_SPACE: core_config::OptId = core_config::opt::SELF_CLOSING_SPACE;
#[allow(dead_code)]
pub const VOID_ELEMENT_STYLE: core_config::OptId = core_config::opt::VOID_ELEMENT_STYLE;
#[allow(dead_code)]
pub const CONTENT_NEWLINE_WITH_ATTRIBUTES: core_config::OptId = core_config::opt::CONTENT_NEWLINE_WITH_ATTRIBUTES;
#[allow(dead_code)]
pub const CONTENT_NEWLINE_WITH_ELEMENTS: core_config::OptId = core_config::opt::CONTENT_NEWLINE_WITH_ELEMENTS;
#[allow(dead_code)]
pub const INDENT_COMMENT_LINES: core_config::OptId = core_config::opt::INDENT_COMMENT_LINES;
#[allow(dead_code)]
pub const INDENT_EXPRESSION_OPERANDS: core_config::OptId = core_config::opt::INDENT_EXPRESSION_OPERANDS;
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddedLanguageFormatting { Auto, Off }
#[allow(dead_code)]
pub fn embedded_language_formatting(o: &core_config::ResolvedOptions) -> EmbeddedLanguageFormatting {
    match o.text(EMBEDDED_LANGUAGE_FORMATTING) {
        "off" => EmbeddedLanguageFormatting::Off,
        _ => EmbeddedLanguageFormatting::Auto,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhitespaceSensitivity { Css, Strict, Ignore }
#[allow(dead_code)]
pub fn whitespace_sensitivity(o: &core_config::ResolvedOptions) -> WhitespaceSensitivity {
    match o.text(WHITESPACE_SENSITIVITY) {
        "strict" => WhitespaceSensitivity::Strict,
        "ignore" => WhitespaceSensitivity::Ignore,
        _ => WhitespaceSensitivity::Css,
    }
}
#[allow(dead_code)]
pub fn bracket_same_line(o: &core_config::ResolvedOptions) -> bool {
    o.flag(BRACKET_SAME_LINE, false)
}
#[allow(dead_code)]
pub fn single_attribute_per_line(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SINGLE_ATTRIBUTE_PER_LINE, false)
}
#[allow(dead_code)]
pub fn max_attributes_per_line(o: &core_config::ResolvedOptions) -> usize {
    o.uint(MAX_ATTRIBUTES_PER_LINE, 0)
}
#[allow(dead_code)]
pub fn self_closing_space(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SELF_CLOSING_SPACE, true)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoidElementStyle { Selfclose, Omit }
#[allow(dead_code)]
pub fn void_element_style(o: &core_config::ResolvedOptions) -> VoidElementStyle {
    match o.text(VOID_ELEMENT_STYLE) {
        "omit" => VoidElementStyle::Omit,
        _ => VoidElementStyle::Selfclose,
    }
}
#[allow(dead_code)]
pub fn content_newline_with_attributes(o: &core_config::ResolvedOptions) -> bool {
    o.flag(CONTENT_NEWLINE_WITH_ATTRIBUTES, false)
}
#[allow(dead_code)]
pub fn content_newline_with_elements(o: &core_config::ResolvedOptions) -> bool {
    o.flag(CONTENT_NEWLINE_WITH_ELEMENTS, false)
}
#[allow(dead_code)]
pub fn indent_comment_lines(o: &core_config::ResolvedOptions) -> bool {
    o.flag(INDENT_COMMENT_LINES, false)
}
#[allow(dead_code)]
pub fn indent_expression_operands(o: &core_config::ResolvedOptions) -> bool {
    o.flag(INDENT_EXPRESSION_OPERANDS, false)
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::line_width(80),
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::choice(core_config::opt::EMBEDDED_LANGUAGE_FORMATTING, &["auto", "off"], "auto", "Format the code of `<script>` and `<style>` and of `style`, `srcset` and event-handler attributes with the formatter of its language (`auto`, Prettier's `embeddedLanguageFormatting`), or keep it as written (`off`). Code that does not parse, or of a language that is not installed, is kept either way."),
        core_config::OptionDef::choice(core_config::opt::WHITESPACE_SENSITIVITY, &["css", "strict", "ignore"], "css", "Whether whitespace between inline elements and text is significant: `css` follows each element's CSS display, `strict` treats every element as inline, `ignore` as block."),
        core_config::OptionDef::boolean(core_config::opt::BRACKET_SAME_LINE, false, "Put the `>` of a multi-line start tag at the end of the last attribute's line."),
        core_config::OptionDef::boolean(core_config::opt::SINGLE_ATTRIBUTE_PER_LINE, false, "Put every attribute of a tag that has several on its own line."),
        core_config::OptionDef::int(core_config::opt::MAX_ATTRIBUTES_PER_LINE, 0, (0, 100), "Put every attribute of a start tag that has more than this many on its own line (`vue/max-attributes-per-line`, `@html-eslint/attrs-newline`); 0 sets no limit. `single_attribute_per_line` is the limit 1."),
        core_config::OptionDef::boolean(core_config::opt::SELF_CLOSING_SPACE, true, "Write a space before the `/>` of a self-closing element (`<use />`, `<br />`); off writes `<use/>` (`@html-eslint/no-extra-spacing-tags`)."),
        core_config::OptionDef::choice(core_config::opt::VOID_ELEMENT_STYLE, &["selfclose", "omit"], "selfclose", "How a void element (`br`, `img`, ...) is written: `selfclose` as `<br />`, `omit` as `<br>` (`void-style`)."),
        core_config::OptionDef::boolean(core_config::opt::CONTENT_NEWLINE_WITH_ATTRIBUTES, false, "Put the content of an element that has attributes on lines of its own, also when the element fits on a line; inline elements and `pre` keep theirs (`vue/singleline-html-element-content-newline` with its defaults)."),
        core_config::OptionDef::boolean(core_config::opt::CONTENT_NEWLINE_WITH_ELEMENTS, false, "Put the children of an element that has an element child on lines of their own, also when they fit on a line; only where the whitespace between them is not significant (`whitespace_sensitivity: ignore`, `@html-eslint/element-newline`)."),
        core_config::OptionDef::boolean(core_config::opt::INDENT_COMMENT_LINES, false, "Put the lines of a multi-line comment one level deeper than the comment and a closing `-->` on a line of its own at the comment's level, whatever indentation they had (`@html-eslint/indent`); the default keeps them as written."),
        core_config::OptionDef::boolean(core_config::opt::INDENT_EXPRESSION_OPERANDS, false, "Indent the operands of a binary or logical expression that continues on following lines in an attribute or `{{ }}` expression (`vue/html-indent`); the default aligns them with the first operand."),
        ])
        .with_presets(vec![
        core_config::Preset::new("html/html-validate-standard", include_str!("../presets/html-validate-standard.yaml")),
        core_config::Preset::new("html/html-validate-recommended", include_str!("../presets/html-validate-recommended.yaml")),
        core_config::Preset::new("html/htmlhint", include_str!("../presets/htmlhint.yaml")),
        core_config::Preset::new("html/html-eslint-recommended", include_str!("../presets/html-eslint-recommended.yaml")),
        core_config::Preset::new("html/angular-eslint-recommended", include_str!("../presets/angular-eslint-recommended.yaml")),
        core_config::Preset::new("html/eslint-plugin-vue-recommended", include_str!("../presets/eslint-plugin-vue-recommended.yaml")),
        core_config::Preset::new("html/eslint-plugin-svelte-recommended", include_str!("../presets/eslint-plugin-svelte-recommended.yaml")),
        ])
    })
}
