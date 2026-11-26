//! The control keywords of Mako: a block, its arms and its closer.
//!
//! concept: none (data of the shared pairing of core_syntax::block_pair)

use core_syntax::block_pair::Block;

pub const BLOCKS: [Block; 5] = [
    Block {
        open: "if",
        arms: &["elif", "else"],
        last_arm: "else",
        close: "endif",
    },
    Block {
        open: "for",
        arms: &["else"],
        last_arm: "else",
        close: "endfor",
    },
    Block {
        open: "while",
        arms: &["else"],
        last_arm: "else",
        close: "endwhile",
    },
    Block {
        open: "try",
        arms: &["except", "else", "finally"],
        last_arm: "finally",
        close: "endtry",
    },
    Block::plain("with", "endwith"),
];
