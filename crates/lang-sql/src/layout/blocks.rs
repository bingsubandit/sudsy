//! The indentation of procedural blocks. The MySQL parse leaves `BEGIN ... END`, `LOOP`,
//! `WHILE` and `REPEAT` as flat statements with no indent, so the indents sqlfluff's dialect has are put
//! on the points here, from the keywords and statements.
//!
//! concept: new indent-balance-layout
//! what: Indentation from the indent and dedent balance of each line.
//! tools: sqlfluff LT02

use super::seq::Seq;
use crate::generated::kind as k;

fn word(seq: &Seq, i: usize) -> String {
    seq.blocks
        .get(i)
        .map_or(String::new(), |b| b.text.to_ascii_uppercase())
}

/// Is block `i` the first word of a statement (after `;`, a label, or an opening keyword)?
fn starts_statement(seq: &Seq, i: usize) -> bool {
    i == 0
        || matches!(seq.blocks[i - 1].kind, k::SEMICOLON | k::COLON)
        || matches!(
            word(seq, i - 1).as_str(),
            "BEGIN" | "LOOP" | "THEN" | "ELSE" | "DO" | "REPEAT"
        )
}

fn indent(seq: &mut Seq, at: usize) {
    if at < seq.points.len() {
        seq.points[at].metas.push(1);
    }
}

fn dedent(seq: &mut Seq, at: usize) {
    seq.points[at].metas.push(-1);
}

pub fn add_indents(seq: &mut Seq) {
    let (mut depth, mut cases) = (0usize, 0usize);
    let (mut do_pending, mut after_until) = (false, false);
    for i in 0..seq.blocks.len() {
        if seq.blocks[i].kind != k::KEYWORD {
            continue;
        }
        let stmt = starts_statement(seq, i);
        let next = word(seq, i + 1);
        match word(seq, i).as_str() {
            "BEGIN" if !matches!(next.as_str(), "WORK" | "TRANSACTION" | "TRAN" | ";" | "") => {
                depth += 1;
                indent(seq, i + 1);
            }
            "LOOP" if word(seq, i.wrapping_sub(1)) != "END" => {
                depth += 1;
                indent(seq, i + 1);
            }
            "REPEAT" if stmt => {
                depth += 1;
                indent(seq, i + 1);
            }
            "WHILE" if stmt && word(seq, i.wrapping_sub(1)) != "END" => do_pending = true,
            "CASE" if stmt => cases += 1,
            "DO" if do_pending => {
                do_pending = false;
                depth += 1;
                indent(seq, i + 1);
            }
            "UNTIL" if stmt && depth > 0 => {
                dedent(seq, i);
                depth -= 1;
                after_until = true;
            }
            "END" if stmt => match next.as_str() {
                "CASE" => cases = cases.saturating_sub(1),
                "REPEAT" if after_until => after_until = false,
                _ if depth > 0 => {
                    dedent(seq, i);
                    depth -= 1;
                }
                _ => {}
            },
            _ => {}
        }
    }
}
