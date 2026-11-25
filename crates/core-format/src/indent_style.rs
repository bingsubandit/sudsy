//! The blanks that indent a line, converted between tabs and spaces on request, and the indentation
//! of a line that continues the one before it. A tab is `indent_size` columns. Both are options of the
//! language (`indent_conversion`: `as_written`, `spaces`, `tabs`; `continued_indent`: `as_written`,
//! `none`); a line continued without a blank before the line break is as written, because the blanks
//! after it may be what separates two words.
//!
//! concept: indent-conversion
//! what: Tabs of the indentation turned into spaces or spaces into tabs, and the indentation of continued lines removed, as options.
//! tools: none (editor setting)
//! reads: option:indent_conversion, option:continued_indent, option:indent_size

use core_config::{ResolvedOptions, opt, shared};

#[derive(Clone, Copy)]
pub struct IndentStyle {
    /// 0 as written, 1 spaces, 2 tabs.
    mode: u8,
    width: usize,
    strip_continued: bool,
}

impl IndentStyle {
    pub fn of(options: &ResolvedOptions) -> IndentStyle {
        IndentStyle {
            mode: match options.text(opt::INDENT_CONVERSION) {
                "spaces" => 1,
                "tabs" => 2,
                _ => 0,
            },
            width: (options.int(shared::INDENT_SIZE) as usize).max(1),
            strip_continued: options.text(opt::CONTINUED_INDENT) == "none",
        }
    }

    /// The indentation `gap` in the style of the option.
    pub fn apply(&self, gap: &str) -> String {
        let mut columns = 0;
        for c in gap.chars() {
            match c {
                ' ' => columns += 1,
                '\t' => columns += self.width - columns % self.width,
                _ => return gap.to_string(),
            }
        }
        match self.mode {
            1 => " ".repeat(columns),
            2 => format!(
                "{}{}",
                "\t".repeat(columns / self.width),
                " ".repeat(columns % self.width)
            ),
            _ => gap.to_string(),
        }
    }

    /// The indentation of a continued line: none when the option says so and a blank stood before the
    /// line break, else as written.
    pub fn continued(&self, gap: &str, blank_before: bool) -> String {
        if self.strip_continued && blank_before {
            String::new()
        } else {
            gap.to_string()
        }
    }
}
