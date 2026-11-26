//! The pairing of blocks that are written as a flat list of commands or statements: a command
//! that opens a block (`if`), the commands that continue it (`elseif`, `else`) and the one that
//! closes it (`endif`), matched by name, case-insensitively. The tree stays flat; a language feeds
//! the names in order and reads each one's role and depth. A block that cannot be paired is
//! reported on its own command and changes nothing else: a closer with no opener is ignored, a
//! closer of an outer block closes the blocks open inside it, a block still open at the end is
//! reported at its opener.
//!
//! concept: keyword-block-pairing
//! what: Pairs opener, arm and closer commands of a flat list by name and reports the unpaired ones.
//! tools: cmake-lint C0307, gersemi indent
//! reads: path:block_pair::Block

/// The words of one kind of block.
#[derive(Debug)]
pub struct Block {
    pub open: &'static str,
    /// Words that continue the block (`elseif`, `else`), each only inside it.
    pub arms: &'static [&'static str],
    /// An arm after which no arm follows (`else`).
    pub last_arm: &'static str,
    pub close: &'static str,
}

impl Block {
    /// A block with no arms.
    pub const fn plain(open: &'static str, close: &'static str) -> Block {
        Block {
            open,
            arms: &[],
            last_arm: "",
            close,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// No block word, or a closer or arm that pairs with nothing.
    Plain,
    Open,
    Arm,
    Close,
}

/// Why a command is not part of a pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// An opener whose closer never comes.
    UnclosedOpener,
    /// A closer with no opener of its kind open.
    UnpairedCloser,
    /// An arm outside a block of its kind.
    UnpairedArm,
    /// An arm after the last arm (`elseif` after `else`).
    ArmAfterLast,
}

impl Fault {
    pub fn code(self) -> &'static str {
        match self {
            Fault::UnclosedOpener => "unclosed-block",
            Fault::UnpairedCloser => "unpaired-closer",
            Fault::ArmAfterLast => "arm-after-last",
            Fault::UnpairedArm => "unpaired-arm",
        }
    }
}

impl Fault {
    pub fn message(self) -> &'static str {
        match self {
            Fault::UnclosedOpener => "this block is never closed",
            Fault::UnpairedCloser => "no block of this kind is open",
            Fault::ArmAfterLast => "no arm may follow the last arm",
            Fault::UnpairedArm => "this arm is outside a block of its kind",
        }
    }
}

/// A faulty command: the `at` it was fed with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Problem {
    pub at: u32,
    pub fault: Fault,
}

/// What [`Pairing::word`] says of a command: its role and the nesting depth it is printed at (an
/// arm and a closer at the depth of their opener).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub role: Role,
    pub depth: u32,
}

impl Step {
    /// The column the command is written at: its depth times the columns of one level. The
    /// layout of a language and the indent lint both take their indent from here.
    pub fn column(self, size: usize) -> usize {
        self.depth as usize * size
    }

    /// The depth of the commands after this one: inside the block an opener or an arm starts.
    pub fn after(self) -> u32 {
        self.depth + matches!(self.role, Role::Open | Role::Arm) as u32
    }
}

struct Open {
    block: u8,
    at: u32,
    last_seen: bool,
}

/// The pairing of one list of commands, fed in order.
pub struct Pairing {
    blocks: &'static [Block],
    /// Bit `c - 'a'` is set for the first letter of every word of the table: most commands are
    /// no block word and cost one test.
    initials: u32,
    stack: Vec<Open>,
    problems: Vec<Problem>,
}

fn is(word: &str, of: &str) -> bool {
    word.eq_ignore_ascii_case(of)
}

impl Pairing {
    pub fn new(blocks: &'static [Block]) -> Pairing {
        let mut initials = 0;
        for b in blocks {
            for w in [b.open, b.close].into_iter().chain(b.arms.iter().copied()) {
                initials |= 1 << (w.as_bytes()[0].to_ascii_lowercase() - b'a');
            }
        }
        Pairing {
            blocks,
            initials,
            stack: Vec::new(),
            problems: Vec::new(),
        }
    }

    fn fault(&mut self, at: u32, fault: Fault) {
        self.problems.push(Problem { at, fault });
    }

    /// The next command, named `word`; `at` is what a [`Problem`] about it reports.
    pub fn word(&mut self, word: &str, at: u32) -> Step {
        let depth = self.stack.len() as u32;
        let plain = Step {
            role: Role::Plain,
            depth,
        };
        let Some(&first) = word.as_bytes().first() else {
            return plain;
        };
        let bit = first.to_ascii_lowercase().wrapping_sub(b'a');
        if bit >= 26 || self.initials >> bit & 1 == 0 {
            return plain;
        }
        let blocks = self.blocks;
        for (i, b) in blocks.iter().enumerate() {
            if is(word, b.open) {
                self.stack.push(Open {
                    block: i as u8,
                    at,
                    last_seen: false,
                });
                return Step {
                    role: Role::Open,
                    depth,
                };
            }
            if is(word, b.close) {
                return self.close(i, at, plain);
            }
            if b.arms.iter().any(|a| is(word, a)) {
                // A word that is an arm of several blocks (`else`) belongs to the open one.
                let open = (self.stack.last())
                    .map(|o| o.block as usize)
                    .filter(|&o| blocks[o].arms.iter().any(|a| is(word, a)));
                return self.arm(open.unwrap_or(i), word, at, plain);
            }
        }
        plain
    }

    /// The next command, which the language reads as a closer (`endfoo`): a word that closes no
    /// block of the table is a closer with no opener ([`Fault::UnpairedCloser`]).
    pub fn closer(&mut self, word: &str, at: u32) -> Step {
        let step = self.word(word, at);
        if !self.blocks.iter().any(|b| is(word, b.close)) {
            self.fault(at, Fault::UnpairedCloser);
        }
        step
    }

    fn arm(&mut self, block: usize, word: &str, at: u32, plain: Step) -> Step {
        let b = &self.blocks[block];
        let Some(top) = self.stack.last_mut().filter(|o| o.block as usize == block) else {
            self.fault(at, Fault::UnpairedArm);
            return plain;
        };
        let after_last = top.last_seen;
        top.last_seen |= is(word, b.last_arm);
        let depth = self.stack.len() as u32 - 1;
        if after_last {
            self.fault(at, Fault::ArmAfterLast);
        }
        Step {
            role: Role::Arm,
            depth,
        }
    }

    fn close(&mut self, block: usize, at: u32, plain: Step) -> Step {
        let Some(from) = self.stack.iter().rposition(|o| o.block as usize == block) else {
            self.fault(at, Fault::UnpairedCloser);
            return plain;
        };
        while self.stack.len() > from + 1 {
            let open = self.stack.pop().unwrap();
            self.fault(open.at, Fault::UnclosedOpener);
        }
        self.stack.pop();
        Step {
            role: Role::Close,
            depth: from as u32,
        }
    }

    /// The opening words of the blocks the next command stands in, innermost first.
    pub fn enclosing(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.stack
            .iter()
            .rev()
            .map(|o| self.blocks[o.block as usize].open)
    }

    /// The problems, in the order found, after the last command: the blocks still open come last.
    pub fn finish(mut self) -> Vec<Problem> {
        while let Some(open) = self.stack.pop() {
            self.fault(open.at, Fault::UnclosedOpener);
        }
        self.problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static BLOCKS: [Block; 4] = [
        Block {
            open: "if",
            arms: &["elseif", "else"],
            last_arm: "else",
            close: "endif",
        },
        Block::plain("foreach", "endforeach"),
        Block::plain("function", "endfunction"),
        Block {
            open: "while",
            arms: &["else"],
            last_arm: "else",
            close: "endwhile",
        },
    ];

    #[test]
    fn a_closer_word_of_no_block_is_unpaired() {
        let mut p = Pairing::new(&BLOCKS);
        assert_eq!(p.closer("endfoo", 4).role, Role::Plain);
        assert_eq!(p.word("if", 5).role, Role::Open);
        assert_eq!(p.closer("endif", 6).role, Role::Close);
        let f: Vec<_> = (p.finish().into_iter()).map(|f| (f.fault, f.at)).collect();
        assert_eq!(f, [(Fault::UnpairedCloser, 4)]);
    }

    #[test]
    fn enclosing_lists_open_blocks_innermost_first() {
        let mut p = Pairing::new(&BLOCKS);
        for w in ["function", "if", "else", "foreach"] {
            p.word(w, 0);
        }
        assert_eq!(
            p.enclosing().collect::<Vec<_>>(),
            ["foreach", "if", "function"]
        );
        p.word("endforeach", 0);
        assert_eq!(p.enclosing().next(), Some("if"));
    }

    /// The roles with their depth as `o0 a0 c0 .` and the faults as `code@at`.
    fn run(words: &str) -> (String, Vec<String>) {
        let mut p = Pairing::new(&BLOCKS);
        let steps: Vec<String> = (words.split(' '))
            .enumerate()
            .map(|(i, w)| {
                let s = p.word(w, i as u32);
                let r = match s.role {
                    Role::Plain => '.',
                    Role::Open => 'o',
                    Role::Arm => 'a',
                    Role::Close => 'c',
                };
                format!("{r}{}", s.depth)
            })
            .collect();
        let faults = (p.finish().iter())
            .map(|f| format!("{}@{}", f.fault.code(), f.at))
            .collect();
        (steps.join(" "), faults)
    }

    #[test]
    fn nested_blocks_have_the_depth_of_their_opener() {
        let (s, f) = run("if foreach set endforeach else set endif set");
        assert_eq!(s, "o0 o1 .2 c1 a0 .1 c0 .0");
        assert!(f.is_empty());
    }

    #[test]
    fn the_words_are_case_insensitive() {
        let (s, f) = run("IF set ElseIf set ELSE set EndIf");
        assert_eq!(s, "o0 .1 a0 .1 a0 .1 c0");
        assert!(f.is_empty());
    }

    #[test]
    fn a_block_inside_a_function() {
        let (s, f) = run("function if set endif foreach endforeach endfunction");
        assert_eq!(s, "o0 o1 .2 c1 o1 c1 c0");
        assert!(f.is_empty());
    }

    #[test]
    fn an_unpaired_closer_is_ignored() {
        let (s, f) = run("set endif set foreach endforeach");
        assert_eq!(s, ".0 .0 .0 o0 c0");
        assert_eq!(f, ["unpaired-closer@1"]);
    }

    #[test]
    fn an_opener_without_closer_is_reported_at_the_opener() {
        let (s, f) = run("set if set");
        assert_eq!(s, ".0 o0 .1");
        assert_eq!(f, ["unclosed-block@1"]);
    }

    #[test]
    fn a_closer_of_an_outer_block_closes_the_blocks_inside() {
        let (s, f) = run("foreach if set endforeach set");
        assert_eq!(s, "o0 o1 .2 c0 .0");
        assert_eq!(f, ["unclosed-block@1"]);
    }

    #[test]
    fn an_arm_word_of_two_blocks_belongs_to_the_open_one() {
        let (s, f) = run("if while else set endwhile else set endif");
        assert_eq!(s, "o0 o1 a1 .2 c1 a0 .1 c0");
        assert!(f.is_empty());
    }

    #[test]
    fn a_wrong_closer_changes_nothing() {
        let (s, f) = run("if set endforeach set endif");
        assert_eq!(s, "o0 .1 .1 .1 c0");
        assert_eq!(f, ["unpaired-closer@2"]);
    }

    #[test]
    fn an_arm_after_the_last_arm_is_reported() {
        let (s, f) = run("if else elseif endif");
        assert_eq!(s, "o0 a0 a0 c0");
        assert_eq!(f, ["arm-after-last@2"]);
        let (_, f) = run("if elseif else endif");
        assert!(f.is_empty());
    }

    #[test]
    fn an_arm_outside_its_block_is_ignored() {
        let (s, f) = run("else foreach else endforeach");
        assert_eq!(s, ".0 o0 .1 c0");
        assert_eq!(f, ["unpaired-arm@0", "unpaired-arm@2"]);
    }
}
