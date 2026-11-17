//! Prism through `ruby-prism-sys`, the one place with FFI. One parse gives everything the adapter
//! needs: the tokens (Prism's lex callback, which neither the sys crate's bindings nor the Rust
//! crate expose, declared by hand), the spans of the nodes (`pm_visit_node`, also declared by
//! hand), the diagnostics and the location of the data after `__END__`. The Rust crate
//! `ruby-prism` is not used: its AST wrappers give no tokens and skip a hook for typed fields.

#![allow(unsafe_code)]

use ruby_prism_sys::{
    pm_arena_free, pm_arena_new, pm_diagnostic_location, pm_diagnostic_message, pm_diagnostic_next,
    pm_node_t, pm_parse, pm_parser_data_loc, pm_parser_errors_first, pm_parser_free, pm_parser_new,
    pm_parser_t,
};
use std::ffi::{CStr, c_char, c_void};
use std::sync::OnceLock;

/// `pm_token_t`.
#[repr(C)]
struct PmToken {
    ty: u32,
    start: *const u8,
    end: *const u8,
}

/// `pm_visit_result_t`.
const DESCEND: u32 = 1;

unsafe extern "C" {
    fn pm_parser_lex_callback_set(
        parser: *mut pm_parser_t,
        callback: extern "C" fn(*mut pm_parser_t, *mut PmToken, *mut c_void),
        data: *mut c_void,
    );
    fn pm_visit_node(
        node: *const pm_node_t,
        visitor: extern "C" fn(*const pm_node_t, *mut c_void) -> u32,
        data: *mut c_void,
    ) -> bool;
    fn pm_token_type(ty: u32) -> *const c_char;
    fn pm_node_type(ty: u16) -> *const c_char;
}

/// A token: its type (`pm_token_type_t`) and span.
#[derive(Clone, Copy)]
pub struct RawToken {
    pub ty: u32,
    pub start: u32,
    pub end: u32,
}

/// A node: its type (`pm_node_type_t`) and span.
#[derive(Clone, Copy)]
pub struct RawNode {
    pub ty: u16,
    pub start: u32,
    pub end: u32,
}

/// A diagnostic: span and message.
pub struct RawError {
    pub start: u32,
    pub len: u32,
    pub message: String,
}

pub struct Parsed {
    /// In lex order: the body of a heredoc comes right after its opener, before the rest of the
    /// opener's line. The end-of-file token and zero-width ones are included.
    pub tokens: Vec<RawToken>,
    /// In pre-order (`ProgramNode` first).
    pub nodes: Vec<RawNode>,
    pub errors: Vec<RawError>,
    /// The start of the `__END__` line (Prism's location of the data covers it).
    pub data: Option<u32>,
}

struct Sink {
    base: *const u8,
    tokens: Vec<RawToken>,
    nodes: Vec<RawNode>,
}

extern "C" fn on_token(_: *mut pm_parser_t, token: *mut PmToken, data: *mut c_void) {
    // SAFETY: Prism passes the pointer given to `pm_parser_lex_callback_set` and a valid token;
    // both live for the duration of `pm_parse`, and `start` and `end` point into the source.
    let (sink, token) = unsafe { (&mut *data.cast::<Sink>(), &*token) };
    let offset = |p: *const u8| (p as usize - sink.base as usize) as u32;
    sink.tokens.push(RawToken {
        ty: token.ty,
        start: offset(token.start),
        end: offset(token.end),
    });
}

extern "C" fn on_node(node: *const pm_node_t, data: *mut c_void) -> u32 {
    // SAFETY: Prism passes the pointer given to `pm_visit_node` and a node of the tree.
    let (sink, node) = unsafe { (&mut *data.cast::<Sink>(), &*node) };
    let (start, length) = (node.location.start, node.location.length);
    sink.nodes.push(RawNode {
        ty: node.type_,
        start,
        end: start + length,
    });
    DESCEND
}

pub fn parse(src: &str) -> Parsed {
    let mut sink = Sink {
        base: src.as_ptr(),
        tokens: Vec::with_capacity(src.len() / 4),
        nodes: Vec::with_capacity(src.len() / 8),
    };
    let (mut errors, mut data) = (Vec::new(), None);
    // SAFETY: the arena and the parser are created, used and freed here; `sink` outlives the
    // calls that use it, the source is valid for `src.len()` bytes, and the tree, the
    // diagnostics and the data location live until the arena is freed.
    unsafe {
        let arena = pm_arena_new();
        let parser = pm_parser_new(arena, src.as_ptr(), src.len(), std::ptr::null());
        pm_parser_lex_callback_set(parser, on_token, (&raw mut sink).cast());
        let root = pm_parse(parser);
        pm_visit_node(root, on_node, (&raw mut sink).cast());
        let mut next = pm_parser_errors_first(parser);
        while !next.is_null() {
            let at = pm_diagnostic_location(next);
            let message = CStr::from_ptr(pm_diagnostic_message(next))
                .to_string_lossy()
                .into_owned();
            errors.push(RawError {
                start: at.start,
                len: at.length,
                message,
            });
            next = pm_diagnostic_next(next);
        }
        let at = &*pm_parser_data_loc(parser);
        if at.length > 0 {
            data = Some(at.start);
        }
        pm_parser_free(parser);
        pm_arena_free(arena);
    }
    Parsed {
        tokens: sink.tokens,
        nodes: sink.nodes,
        errors,
        data,
    }
}

/// The number of token types of Prism 2.1.0 (`PM_TOKEN_EOF` = 1 to `PM_TOKEN___END__`).
const TOKEN_TYPES: u32 = 170;
/// The number of node types (`PM_ALIAS_GLOBAL_VARIABLE_NODE` = 1 to `PM_YIELD_NODE`).
const NODE_TYPES: u16 = 152;

/// The names of the token types, index = type (`BRACE_RIGHT`; `__END__`).
pub fn token_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let name = |ty| {
            // SAFETY: `pm_token_type` returns a static string for each type from 1 to `TOKEN_TYPES`.
            (ty != 0).then(|| {
                unsafe { CStr::from_ptr(pm_token_type(ty)) }
                    .to_string_lossy()
                    .trim_matches('\'')
                    .to_string()
            })
        };
        (0..=TOKEN_TYPES)
            .map(|ty| name(ty).unwrap_or_default())
            .collect()
    })
}

/// The names of the node types as Prism's docs write them, index = type (`CallNode`).
pub fn node_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let name = |ty| {
            // SAFETY: `pm_node_type` returns a static `PM_CALL_NODE` style string for each type
            // from 1 to `NODE_TYPES`.
            let upper = unsafe { CStr::from_ptr(pm_node_type(ty)) }.to_string_lossy();
            upper
                .trim_start_matches("PM_")
                .split('_')
                .map(pascal)
                .collect::<String>()
        };
        (0..=NODE_TYPES)
            .map(|ty| if ty == 0 { String::new() } else { name(ty) })
            .collect()
    })
}

fn pascal(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or(String::new(), |c| {
        c.to_string() + &chars.as_str().to_ascii_lowercase()
    })
}
