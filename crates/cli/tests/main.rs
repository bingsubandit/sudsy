//! The crate's integration tests: one test binary instead of one per file.

#[cfg(not(feature = "pack-mode"))]
mod builtin;
mod config;
mod debug;
mod format;
mod init;
#[cfg(feature = "pack-mode")]
mod languages;
mod layout;
mod lint;
mod lsp;
mod nesting;
mod new;
mod patterns;
mod presets;
mod report;
mod semantic;
