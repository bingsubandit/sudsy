//! The tag table: void elements and which start tag closes an open element (HTML's optional end
//! tags, as `angular-html-parser` reads them). Names are lowercase.

pub fn is_void(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "basefont"
            | "bgsound"
            | "br"
            | "col"
            | "command"
            | "embed"
            | "frame"
            | "hr"
            | "image"
            | "img"
            | "input"
            | "isindex"
            | "keygen"
            | "link"
            | "menuitem"
            | "meta"
            | "nextid"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// Does the start tag `child` end the open element `parent` (`<li>a<li>`)?
pub fn closes(parent: &str, child: &str) -> bool {
    match parent {
        "li" => child == "li",
        "dt" | "dd" => matches!(child, "dt" | "dd"),
        "p" => matches!(
            child,
            "address"
                | "article"
                | "aside"
                | "blockquote"
                | "div"
                | "dl"
                | "fieldset"
                | "footer"
                | "form"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "header"
                | "hgroup"
                | "hr"
                | "main"
                | "nav"
                | "ol"
                | "p"
                | "pre"
                | "section"
                | "table"
                | "ul"
        ),
        "thead" => matches!(child, "tbody" | "tfoot"),
        "tbody" => matches!(child, "tbody" | "tfoot"),
        "tfoot" => child == "tbody",
        "tr" => matches!(child, "tr" | "tbody" | "tfoot"),
        "td" | "th" => matches!(child, "td" | "th" | "tr"),
        "rb" | "rt" | "rp" => matches!(child, "rb" | "rt" | "rtc" | "rp"),
        "rtc" => matches!(child, "rb" | "rtc" | "rp"),
        "optgroup" => child == "optgroup",
        "option" => matches!(child, "option" | "optgroup"),
        _ => false,
    }
}
