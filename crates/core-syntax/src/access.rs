//! The parse of access chains over a cursor: a base followed by subscripts (`a[1]`), members
//! (`a.b`) and the other suffixes a language lets follow (`++`, `::`), the items of a subscript
//! (`[1:5]`, `[:]`, `[1, 2]`) and the keyword-or-name decision of a word that may qualify a name.
//! The language supplies its tokens, its node kinds and its element parser through the traits;
//! the loops are shared.
//!
//! concept: access-chain-parse
//! what: Loops of subscripts, members and suffixes, subscript items with slices, and keyword words that are names when qualified.
//! tools: none (parser plumbing)
//! reads: path:access::Access, path:access::Subscript

/// The parser of a language as an access chain needs it.
pub trait Access {
    /// A subscript opens at the cursor (`[`).
    fn at_subscript(&self) -> bool;
    /// Takes the subscript, brackets included.
    fn subscript(&mut self);
    /// A member access starts at the cursor (`.` and a name).
    fn at_member(&self) -> bool;
    /// Takes the member; false when the chain ends with it (`t.*`) or it is none of the
    /// languages's members after all.
    fn member(&mut self) -> bool;
    /// Takes another suffix of the language (`++`, `::`); false when none is at the cursor.
    fn suffix(&mut self) -> bool;
}

/// Takes subscripts, members and suffixes while they follow each other.
pub fn access_chain<C: Access + ?Sized>(c: &mut C) {
    loop {
        if c.at_subscript() {
            c.subscript();
        } else if c.at_member() {
            if !c.member() {
                return;
            }
        } else if !c.suffix() {
            return;
        }
    }
}

/// The parser of a language as the items of a subscript need it.
pub trait Subscript {
    /// The separator of the bounds of a slice is at the cursor (`:`).
    fn at_slice(&self) -> bool;
    /// A comma is at the cursor.
    fn at_comma(&self) -> bool;
    fn bump_slice(&mut self);
    fn bump_comma(&mut self);
    /// One element; false when none is at the cursor.
    fn element(&mut self) -> bool;
}

/// The items of a subscript up to its closing bracket: elements apart by commas and slice
/// separators, the ends of a slice may be empty (`[:5]`, `[1:]`, `[:]`).
pub fn subscript_items<C: Subscript + ?Sized>(c: &mut C) {
    loop {
        if c.at_slice() {
            c.bump_slice();
            continue;
        }
        if !c.element() || !(c.at_comma() || c.at_slice()) {
            return;
        }
        if c.at_comma() {
            c.bump_comma();
        }
    }
}

/// Is `text` one of the keyword `words` here? A word directly followed by a qualifier separator
/// (`unbounded.x`) is a name, not a keyword.
pub fn is_keyword_word(words: &[&str], text: &str, qualifier_follows: bool) -> bool {
    !qualifier_follows && words.iter().any(|w| text.eq_ignore_ascii_case(w))
}
