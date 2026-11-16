// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const WRAP_LINES: &str = "wrap_lines";
#[allow(dead_code)]
pub const CONTINUATION_INDENT: &str = "continuation_indent";
#[allow(dead_code)]
pub const ACCESS_MODIFIER_OFFSET: &str = "access_modifier_offset";
#[allow(dead_code)]
pub const INDENT_CASE_LABELS: &str = "indent_case_labels";
#[allow(dead_code)]
pub const NAMESPACE_INDENTATION: &str = "namespace_indentation";
#[allow(dead_code)]
pub const POINTER_ALIGNMENT: &str = "pointer_alignment";
#[allow(dead_code)]
pub const BRACE_WRAPPING: &str = "brace_wrapping";
#[allow(dead_code)]
pub const INDENT_BRACES: &str = "indent_braces";
#[allow(dead_code)]
pub const SHORT_FUNCTIONS: &str = "short_functions";
#[allow(dead_code)]
pub const SHORT_IFS: &str = "short_ifs";
#[allow(dead_code)]
pub const SHORT_LOOPS: &str = "short_loops";
#[allow(dead_code)]
pub const SHORT_LAMBDAS: &str = "short_lambdas";
#[allow(dead_code)]
pub const SHORT_CASE_LABELS: &str = "short_case_labels";
#[allow(dead_code)]
pub const BIN_PACK_ARGUMENTS: &str = "bin_pack_arguments";
#[allow(dead_code)]
pub const BIN_PACK_PARAMETERS: &str = "bin_pack_parameters";
#[allow(dead_code)]
pub const ALIGN_AFTER_OPEN_BRACKET: &str = "align_after_open_bracket";
#[allow(dead_code)]
pub const BREAK_BEFORE_BINARY_OPERATORS: &str = "break_before_binary_operators";
#[allow(dead_code)]
pub const BREAK_CONSTRUCTOR_INITIALIZERS: &str = "break_constructor_initializers";
#[allow(dead_code)]
pub const CONSTRUCTOR_INITIALIZER_INDENT: &str = "constructor_initializer_indent";
#[allow(dead_code)]
pub const BREAK_AFTER_RETURN_TYPE: &str = "break_after_return_type";
#[allow(dead_code)]
pub const SHORT_ENUMS: &str = "short_enums";
#[allow(dead_code)]
pub const PENALTY_RETURN_TYPE: &str = "penalty_return_type";
#[allow(dead_code)]
pub const SPACE_BEFORE_PARENS: &str = "space_before_parens";
#[allow(dead_code)]
pub const SPACES_BEFORE_TRAILING_COMMENTS: &str = "spaces_before_trailing_comments";
#[allow(dead_code)]
pub const ALIGN_TRAILING_COMMENTS: &str = "align_trailing_comments";
#[allow(dead_code)]
pub const BREAK_TEMPLATE_DECLARATIONS: &str = "break_template_declarations";
#[allow(dead_code)]
pub const ALIGN_ESCAPED_NEWLINES: &str = "align_escaped_newlines";

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::line_width(80),
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between statements and comments."),
        core_config::OptionDef::boolean("wrap_lines", true, "Break lines that are longer than `line_width`. When false, every line keeps the line breaks of the source (only indentation and spacing change)."),
        core_config::OptionDef::int("continuation_indent", 4, (0, 16), "Columns a wrapped line is indented more than the line it continues, where it is not aligned to a bracket."),
        core_config::OptionDef::int("access_modifier_offset", -2, (-16, 16), "Columns `public:`, `private:` and `protected:` are indented relative to the members of their class."),
        core_config::OptionDef::boolean("indent_case_labels", false, "Indent `case` labels one level inside their `switch`."),
        core_config::OptionDef::choice("namespace_indentation", &["none", "inner", "all"], "none", "Indent the contents of namespaces: `none`, only namespaces nested in another (`inner`), or `all`."),
        core_config::OptionDef::choice("pointer_alignment", &["right", "left", "middle"], "right", "Where the `*` and `&` of a pointer or reference declarator go: next to the name (`right`), next to the type (`left`) or between them (`middle`)."),
        core_config::OptionDef::words("brace_wrapping", "Constructs whose opening brace goes on its own line: `class`, `struct`, `union`, `enum`, `function`, `namespace`, `extern`, `control` (if, for, while, switch, do, try), `case`, `else` (the `else` after a block starts a line), `catch` (the `catch` after a block starts a line), `while` (the `while` of a do loop starts a line). Empty: every brace stays on the line of its construct."),
        core_config::OptionDef::boolean("indent_braces", false, "Indent a brace that is on its own line together with the block it opens."),
        core_config::OptionDef::choice("short_functions", &["none", "empty", "inline", "all"], "all", "Function bodies kept on the line of the signature when they fit: `none`, `empty` (`{}` only), `inline` (functions defined in a class, and empty ones) or `all`."),
        core_config::OptionDef::choice("short_ifs", &["never", "without_else", "all"], "never", "An `if` with a simple statement is kept on one line when it fits: `never`, without an `else`, or with or without."),
        core_config::OptionDef::boolean("short_loops", false, "A `for`, `while` or range-for with a simple statement is kept on one line when it fits."),
        core_config::OptionDef::choice("short_lambdas", &["none", "empty", "inline", "all"], "all", "Lambda bodies kept on the line of the lambda when they fit: `none`, `empty`, `inline` (a lambda that is an argument) or `all`."),
        core_config::OptionDef::boolean("short_case_labels", false, "A `case` label and its simple statement are kept on one line when they fit."),
        core_config::OptionDef::boolean("bin_pack_arguments", true, "Call arguments that do not fit on a line are filled; when false they go one per line (or all on one line)."),
        core_config::OptionDef::boolean("bin_pack_parameters", true, "Function parameters that do not fit on a line are filled; when false they go one per line (or all on one line)."),
        core_config::OptionDef::choice("align_after_open_bracket", &["align", "dont_align", "always_break"], "align", "Wrapped lines inside brackets line up with the first token after the bracket (`align`), continue with `continuation_indent` (`dont_align`), or always start on a new line after the bracket (`always_break`)."),
        core_config::OptionDef::choice("break_before_binary_operators", &["none", "non_assignment", "all"], "none", "A wrapped binary expression breaks after the operator (`none`), before it except for assignments (`non_assignment`) or before every operator (`all`)."),
        core_config::OptionDef::choice("break_constructor_initializers", &["before_colon", "before_comma"], "before_colon", "Constructor initializer lists that do not fit start on a new line before the colon (`before_colon`, one initializer per line when needed), or put a comma before each initializer, one per line (`before_comma`)."),
        core_config::OptionDef::int("constructor_initializer_indent", 4, (0, 16), "Columns a constructor initializer list is indented more than its constructor."),
        core_config::OptionDef::choice("break_after_return_type", &["none", "top_level", "all", "top_level_definitions", "all_definitions"], "none", "The function name goes on a new line after its return type: never, for declarations and definitions outside classes (`top_level`), for all, or for definitions only (`top_level_definitions`, `all_definitions`)."),
        core_config::OptionDef::boolean("short_enums", true, "An enum whose body fits stays on one line."),
        core_config::OptionDef::int("penalty_return_type", 60, (0, 1000), "What a return type on a line of its own costs the layout; the higher, the longer the parameter list may wrap before the return type moves."),
        core_config::OptionDef::choice("space_before_parens", &["never", "control_statements", "always"], "control_statements", "A blank before the parenthesis of `if`, `for`, `while`, `switch` and `catch` (`control_statements`), before every parenthesis (`always`) or never."),
        core_config::OptionDef::int("spaces_before_trailing_comments", 1, (0, 16), "Blanks between the code of a line and its `//` comment."),
        core_config::OptionDef::boolean("align_trailing_comments", true, "Align the trailing comments of consecutive lines."),
        core_config::OptionDef::choice("break_template_declarations", &["yes", "multiline", "no"], "multiline", "`template <...>` ends its line: always (`yes`), when the declaration is not on one line (`multiline`) or never."),
        core_config::OptionDef::choice("align_escaped_newlines", &["right", "left", "dont_align"], "right", "The backslashes of a multi-line macro: aligned at `line_width` (`right`), one blank after the longest line (`left`) or one blank after each line."),
    ])
    .with_presets(vec![
        core_config::Preset::new("cpp/google", include_str!("../presets/google.yaml")),
        core_config::Preset::new("cpp/chromium", include_str!("../presets/chromium.yaml")),
        core_config::Preset::new("cpp/mozilla", include_str!("../presets/mozilla.yaml")),
        core_config::Preset::new("cpp/webkit", include_str!("../presets/webkit.yaml")),
        core_config::Preset::new("cpp/gnu", include_str!("../presets/gnu.yaml")),
        core_config::Preset::new("cpp/microsoft", include_str!("../presets/microsoft.yaml")),
    ])
}
