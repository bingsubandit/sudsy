//! The indentation of procedural blocks. The MySQL parse leaves `BEGIN ... END`, `LOOP`, `WHILE`,
//! `REPEAT`, `IF` and `CASE` as flat statements with no indent, so the indents sqlfluff's dialect
//! has are put on the points here: the words are this table, the shared frames do the rest.
//!
//! concept: none (glue: the words of the frames, applied to the points)

use super::seq::Seq;
use crate::generated::kind as k;
use core_format::block_frames::{Arm, Frame, Hanging, NameKind, Spec, Tok, points};

pub static SPEC: Spec = Spec {
    statement_after: &["BEGIN", "LOOP", "THEN", "ELSE", "DO", "REPEAT"],
    frames: &[
        Frame {
            open: "BEGIN",
            not_before: &["WORK", "TRANSACTION", "TRAN", ";", ""],
            ..Frame::NONE
        },
        Frame {
            open: "LOOP",
            not_after: &["END"],
            ..Frame::NONE
        },
        Frame {
            open: "REPEAT",
            statement_start: true,
            until: Some("UNTIL"),
            ..Frame::NONE
        },
        Frame {
            open: "WHILE",
            statement_start: true,
            not_after: &["END"],
            body_after: Some("DO"),
            ..Frame::NONE
        },
        Frame {
            open: "IF",
            statement_start: true,
            body_after: Some("THEN"),
            arms: &[
                Arm {
                    word: "ELSEIF",
                    body_after: Some("THEN"),
                },
                Arm {
                    word: "ELSE",
                    body_after: None,
                },
            ],
            ..Frame::NONE
        },
        Frame {
            open: "CASE",
            statement_start: true,
            arms: &[
                Arm {
                    word: "WHEN",
                    body_after: Some("THEN"),
                },
                Arm {
                    word: "ELSE",
                    body_after: None,
                },
            ],
            arm_indent: true,
            ..Frame::NONE
        },
    ],
    end: "END",
    hanging: &[Hanging {
        head: &["ALTER", "TABLE"],
        optional: &["IF", "EXISTS", "ONLY"],
    }],
};

/// Puts the indents of the flat blocks (`blocks`) and of the hanging statements (`hanging`) on
/// the points.
pub fn add_indents(seq: &mut Seq, blocks: bool, hanging: bool) {
    let toks: Vec<Tok> = seq
        .blocks
        .iter()
        .map(|b| Tok {
            text: &b.text,
            keyword: b.kind == k::KEYWORD,
            separator: matches!(b.kind, k::SEMICOLON | k::COLON),
            name: match b.kind {
                k::DOT => NameKind::Dot,
                k::IDENTIFIER | k::WORD | k::QUOTED_IDENTIFIER => NameKind::Part,
                _ => NameKind::None,
            },
        })
        .collect();
    let found = points(&toks, &SPEC, blocks, hanging);
    for (at, meta) in found {
        if let Some(point) = seq.points.get_mut(at) {
            point.metas.push(meta);
        }
    }
}
