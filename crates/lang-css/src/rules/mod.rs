//! The rules of CSS, SCSS and Less: pattern files in `rules/*.yaml`, and the families they name.
//!
//! concept: none (glue: rule registration)

mod checks;
mod font;
mod max;
mod model;
mod names;
mod table;
mod values;

use core_rules::Family;

pub static FAMILIES: &[Family] = &[
    names::FAMILY,
    core_rules::max::FAMILY,
    core_rules::blank_lines::FAMILY,
    max::FAMILY,
    checks::FAMILY,
    table::FAMILY,
];
