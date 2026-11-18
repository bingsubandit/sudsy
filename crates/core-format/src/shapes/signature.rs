use super::list::{ListLayout, ListStyle, TrailingComma, delimited_list};
use crate::FormatCtx;
use crate::doc::{self, Doc, GroupId};
use crate::error::FormatError;
use core_syntax::SyntaxNode;

/// A declaration's signature: `<T> Type name(params) throws A`, laid out as one group.
pub struct Signature {
    /// The type parameters (`<T>`) in front of the head.
    pub type_params: Option<Doc>,
    /// What stands between the type parameters and the parameters (return type, name). When
    /// type parameters stand in front, a head that does not fit moves below them.
    pub head: Option<Doc>,
    /// The parameter list, and the group of its items (see [`ListStyle::inner_id`]).
    pub params: Doc,
    pub params_id: Option<GroupId>,
    /// Clauses after the parameters (`throws`): each stays on the line of the `)` when it fits and
    /// otherwise, or when the parameters are one per line, goes to a line of its own.
    pub tail: Vec<Doc>,
    /// Indentation levels of the continuation lines.
    pub continuation: usize,
}

fn cont(levels: usize, d: Doc) -> Doc {
    (0..levels).fold(d, |d, _| doc::indent(d))
}

pub fn signature(s: Signature) -> Doc {
    let moved = doc::new_group_id();
    let mut parts = Vec::new();
    let hung = s.type_params.is_some() && s.head.is_some();
    match (s.type_params, s.head) {
        (Some(tp), Some(head)) => {
            parts.push(tp);
            parts.push(doc::group_with_id(
                moved,
                cont(s.continuation, doc::concat(vec![doc::line(), head])),
            ));
        }
        (tp, head) => parts.extend(tp.into_iter().chain(head)),
    }
    parts.push(if hung {
        let params = doc::shared(s.params);
        doc::if_break_in(moved, cont(s.continuation, params.clone()), params)
    } else {
        s.params
    });
    for t in s.tail {
        let t = doc::shared(t);
        let own_line = doc::concat(vec![doc::hardline(), t.clone()]);
        let fits = doc::group(doc::concat(vec![doc::line(), t]));
        parts.push(cont(
            s.continuation,
            match s.params_id {
                Some(id) => doc::if_break_in(id, own_line, fits),
                None => fits,
            },
        ));
    }
    doc::group(doc::concat(parts))
}

/// The list of parameters of a signature: one per line from `min_params` of them on (`usize::MAX`:
/// never), else `otherwise`.
pub fn parameter_list(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    min_params: usize,
    trailing_comma: TrailingComma,
    otherwise: ListLayout,
) -> Result<Doc, FormatError> {
    let count = node
        .children()
        .filter(|c| !c.text_range().is_empty())
        .count();
    let style = ListStyle {
        trailing_comma,
        layout: if count >= min_params {
            ListLayout::Break
        } else {
            otherwise
        },
        ..ListStyle::default()
    };
    delimited_list(cx, node, &style)
}
