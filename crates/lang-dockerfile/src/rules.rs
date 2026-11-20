//! The Rust rules of Dockerfile and the families its pattern files name; rules that fit a pattern
//! are files in `rules/`.
//!
//! concept: none (glue: rule registration)

mod run_command;
mod run_stage;
mod stages;

use core_rules::{Family, Rule};

pub static RULES: &[&dyn Rule] = &[
    &stages::UntaggedBaseImage,
    &stages::LastUserNotRoot,
    &stages::CopyFromUnknownStage,
    &stages::CopyFromOwnStage,
    &stages::FromFirst,
];

pub static FAMILIES: &[Family] = &[
    run_command::FAMILY,
    run_stage::CONSECUTIVE,
    run_stage::PIPE_SHELL,
    stages::ONCE_PER_STAGE,
    core_rules::unique::FAMILY,
];
