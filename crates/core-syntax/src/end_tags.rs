//! The pairing of end tags with start tags of a markup language, by name: the open tags of a
//! parser in a stack (an end tag closes the innermost open tag of its name and the tags open
//! inside it), and the words of the error for an end tag that closes another tag.
//!
//! concept: end-tag-pairing
//! what: Matches end tags to open start tags by name and words the mismatch.
//! tools: xmllint, html-validate close-order
//! reads: path:end_tags::Syntax

/// How a language writes its start and end tags: `<` and `</` of XML, `<%` and `</%` of Mako.
pub struct Syntax {
    pub start: &'static str,
    pub end: &'static str,
}

impl Syntax {
    /// The error for the end tag `end_name` that closes the tag `open_name`: `</b>` closes `<a>`.
    pub fn mismatch(&self, open_name: &str, end_name: &str) -> String {
        let (start, end) = (self.start, self.end);
        format!("`{end}{end_name}>` closes `{start}{open_name}>`")
    }
}

/// The open start tags, each with the data its parser keeps for it.
pub struct OpenTags<'a, T>(Vec<(&'a str, T)>);

/// An end tag that found its start tag.
pub struct Closed<'a, T> {
    pub name: &'a str,
    pub data: T,
    /// The tags that were open inside it, innermost first: closed with it, so a mismatch.
    pub inner: Vec<(&'a str, T)>,
}

impl<'a, T> OpenTags<'a, T> {
    pub fn new() -> Self {
        OpenTags(Vec::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn push(&mut self, name: &'a str, data: T) {
        self.0.push((name, data));
    }

    /// Closes the innermost open tag called `name` and the tags inside it; `None` when no open
    /// tag has that name (the stack stays).
    pub fn close(&mut self, name: &str) -> Option<Closed<'a, T>> {
        let at = self.0.iter().rposition(|t| t.0 == name)?;
        let mut inner = self.0.split_off(at + 1);
        inner.reverse();
        let (name, data) = self.0.pop()?;
        Some(Closed { name, data, inner })
    }

    /// The name of the innermost open tag.
    pub fn innermost(&self) -> Option<&'a str> {
        self.0.last().map(|t| t.0)
    }

    /// The tags still open at the end, innermost first.
    pub fn drain(&mut self) -> impl Iterator<Item = (&'a str, T)> + '_ {
        self.0.drain(..).rev()
    }
}

impl<T> Default for OpenTags<'_, T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_end_tag_closes_the_innermost_tag_of_its_name_and_what_is_inside() {
        let mut tags = OpenTags::new();
        for (i, name) in ["a", "b", "a", "c"].into_iter().enumerate() {
            tags.push(name, i);
        }
        assert!(tags.close("x").is_none());
        let closed = tags.close("a").unwrap();
        assert_eq!((closed.name, closed.data), ("a", 2));
        assert_eq!(closed.inner, [("c", 3)]);
        assert_eq!(tags.innermost(), Some("b"));
        let rest: Vec<_> = tags.drain().collect();
        assert_eq!(rest, [("b", 1), ("a", 0)]);
        assert!(tags.is_empty());
    }

    #[test]
    fn a_mismatch_names_both_tags() {
        let xml = Syntax {
            start: "<",
            end: "</",
        };
        assert_eq!(xml.mismatch("a", "b"), "`</b>` closes `<a>`");
    }
}
