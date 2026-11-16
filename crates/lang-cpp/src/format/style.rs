//! The options of the formatter, read once per format.

use super as o;
use core_config::{OptionValue, ResolvedOptions};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PtrAlign {
    Right,
    Left,
    Middle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BracketAlign {
    Align,
    DontAlign,
    AlwaysBreak,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BinBreak {
    None,
    NonAssignment,
    All,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Init {
    BeforeColon,
    BeforeComma,
}

/// The constructs whose brace goes on its own line.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Wrap {
    Class,
    Struct,
    Union,
    Enum,
    Function,
    Namespace,
    Extern,
    Control,
    Case,
    Else,
    Catch,
    While,
}

pub struct Style {
    pub width: usize,
    pub indent: usize,
    pub tabs: bool,
    pub blank_lines: usize,
    pub wrap_lines: bool,
    pub continuation: usize,
    pub access_offset: i64,
    pub case_labels: bool,
    pub namespace_indent: String,
    pub pointer: PtrAlign,
    pub wraps: Vec<Wrap>,
    pub indent_braces: bool,
    pub short_functions: String,
    pub short_ifs: String,
    pub short_loops: bool,
    pub short_lambdas: String,
    pub short_cases: bool,
    pub bin_args: bool,
    pub bin_params: bool,
    pub align: BracketAlign,
    pub bin_break: BinBreak,
    pub init: Init,
    pub init_indent: usize,
    pub parens: String,
    pub comment_spaces: usize,
    pub align_comments: bool,
    pub templates: String,
    pub short_enums: bool,
    pub ret_penalty: u32,
    pub ret_break: String,
    pub escaped: String,
}

fn string<'a>(options: &'a ResolvedOptions, name: &str) -> &'a str {
    match options.get(name) {
        Some(OptionValue::String(s)) => s,
        _ => "",
    }
}

impl Style {
    pub fn new(options: &ResolvedOptions) -> Style {
        let wraps = options
            .strings(o::BRACE_WRAPPING)
            .iter()
            .filter_map(|w| {
                Some(match w.as_str() {
                    "class" => Wrap::Class,
                    "struct" => Wrap::Struct,
                    "union" => Wrap::Union,
                    "enum" => Wrap::Enum,
                    "function" => Wrap::Function,
                    "namespace" => Wrap::Namespace,
                    "extern" => Wrap::Extern,
                    "control" => Wrap::Control,
                    "case" => Wrap::Case,
                    "else" => Wrap::Else,
                    "catch" => Wrap::Catch,
                    "while" => Wrap::While,
                    _ => return None,
                })
            })
            .collect();
        let s = |name| string(options, name).to_string();
        Style {
            width: options.int("line_width") as usize,
            indent: options.int("indent_size") as usize,
            tabs: string(options, "indent_style") == "tab",
            blank_lines: options.max_blank_lines(),
            wrap_lines: options.bool(o::WRAP_LINES),
            continuation: options.int(o::CONTINUATION_INDENT) as usize,
            access_offset: options.int(o::ACCESS_MODIFIER_OFFSET),
            case_labels: options.bool(o::INDENT_CASE_LABELS),
            namespace_indent: s(o::NAMESPACE_INDENTATION),
            pointer: match string(options, o::POINTER_ALIGNMENT) {
                "left" => PtrAlign::Left,
                "middle" => PtrAlign::Middle,
                _ => PtrAlign::Right,
            },
            wraps,
            indent_braces: options.bool(o::INDENT_BRACES),
            short_functions: s(o::SHORT_FUNCTIONS),
            short_ifs: s(o::SHORT_IFS),
            short_loops: options.bool(o::SHORT_LOOPS),
            short_lambdas: s(o::SHORT_LAMBDAS),
            short_cases: options.bool(o::SHORT_CASE_LABELS),
            bin_args: options.bool(o::BIN_PACK_ARGUMENTS),
            bin_params: options.bool(o::BIN_PACK_PARAMETERS),
            align: match string(options, o::ALIGN_AFTER_OPEN_BRACKET) {
                "dont_align" => BracketAlign::DontAlign,
                "always_break" => BracketAlign::AlwaysBreak,
                _ => BracketAlign::Align,
            },
            bin_break: match string(options, o::BREAK_BEFORE_BINARY_OPERATORS) {
                "non_assignment" => BinBreak::NonAssignment,
                "all" => BinBreak::All,
                _ => BinBreak::None,
            },
            init: match string(options, o::BREAK_CONSTRUCTOR_INITIALIZERS) {
                "before_comma" => Init::BeforeComma,
                _ => Init::BeforeColon,
            },
            init_indent: options.int(o::CONSTRUCTOR_INITIALIZER_INDENT) as usize,
            parens: s(o::SPACE_BEFORE_PARENS),
            comment_spaces: options.int(o::SPACES_BEFORE_TRAILING_COMMENTS) as usize,
            align_comments: options.bool(o::ALIGN_TRAILING_COMMENTS),
            templates: s(o::BREAK_TEMPLATE_DECLARATIONS),
            short_enums: options.bool(o::SHORT_ENUMS),
            ret_penalty: options.int(o::PENALTY_RETURN_TYPE) as u32,
            ret_break: s(o::BREAK_AFTER_RETURN_TYPE),
            escaped: s(o::ALIGN_ESCAPED_NEWLINES),
        }
    }

    pub fn wraps(&self, w: Wrap) -> bool {
        self.wraps.contains(&w)
    }
}
