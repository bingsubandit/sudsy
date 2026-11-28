//! Appends to a JSON text without touching the rest of it: the order of the user's keys, their
//! indentation and their line breaks stay. The text must be valid JSON.

use serde::Serialize;
use serde_json::Value;

/// One member of an object or item of an array.
struct Child {
    /// Where its key (or, in an array, its value) starts.
    at: usize,
    /// Where its value ends.
    end: usize,
}

struct Container {
    children: Vec<Child>,
    close: usize,
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while b.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    i
}

/// The index after the string that starts at `i`.
fn string_end(b: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < b.len() && b[i] != b'"' {
        i += if b[i] == b'\\' { 2 } else { 1 };
    }
    i + 1
}

fn value_end(b: &[u8], i: usize) -> usize {
    match b.get(i) {
        Some(b'"') => string_end(b, i),
        Some(b'{' | b'[') => container(b, i).close + 1,
        _ => {
            let mut i = i;
            while i < b.len() && !matches!(b[i], b',' | b'}' | b']') && !b[i].is_ascii_whitespace()
            {
                i += 1;
            }
            i
        }
    }
}

/// The children of the object or array that opens at `open`.
fn container(b: &[u8], open: usize) -> Container {
    let (object, close) = (b[open] == b'{', if b[open] == b'{' { b'}' } else { b']' });
    let mut children = Vec::new();
    let mut i = skip_ws(b, open + 1);
    while i < b.len() && b[i] != close {
        let at = i;
        if object {
            i = skip_ws(b, string_end(b, i));
            i = skip_ws(b, i + 1);
        }
        let end = value_end(b, i);
        children.push(Child { at, end });
        i = skip_ws(b, end);
        if b.get(i) == Some(&b',') {
            i = skip_ws(b, i + 1);
        }
    }
    Container { children, close: i }
}

/// The whitespace in front of `pos` on its line, when nothing else is.
fn line_indent(text: &str, pos: usize) -> Option<&str> {
    let start = text[..pos].rfind('\n').map_or(0, |n| n + 1);
    let indent = &text[start..pos];
    indent.trim().is_empty().then_some(indent)
}

/// The leading whitespace of the line `pos` is on.
fn base_indent(text: &str, pos: usize) -> &str {
    let start = text[..pos].rfind('\n').map_or(0, |n| n + 1);
    let line = &text[start..];
    &line[..line.len() - line.trim_start().len()]
}

/// `value` as JSON with `unit` as one level of indentation and `lead` before every later line.
fn render(value: &Value, unit: &str, lead: &str) -> String {
    let mut out = Vec::new();
    let format = serde_json::ser::PrettyFormatter::with_indent(unit.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut out, format);
    value.serialize(&mut ser).expect("json");
    String::from_utf8(out)
        .expect("utf8")
        .replace('\n', &format!("\n{lead}"))
}

/// The start of the root value.
pub fn root(text: &str) -> usize {
    skip_ws(text.as_bytes(), 0)
}

/// The start of the value of member `key` of the object at `open`.
pub fn member(text: &str, open: usize, key: &str) -> Option<usize> {
    let b = text.as_bytes();
    container(b, open).children.iter().find_map(|c| {
        let end = string_end(b, c.at);
        let name: String = serde_json::from_str(&text[c.at..end]).ok()?;
        (name == key).then(|| skip_ws(b, skip_ws(b, end) + 1))
    })
}

/// Adds `"key": value` as the last member of the object at `open`.
pub fn add_member(text: &str, open: usize, key: &str, value: &Value) -> String {
    let c = container(text.as_bytes(), open);
    let base = base_indent(text, open);
    let members = match c.children.first().and_then(|f| line_indent(text, f.at)) {
        Some(i) => i.to_string(),
        None => format!("{base}  "),
    };
    let unit = members
        .strip_prefix(base)
        .filter(|u| !u.is_empty())
        .unwrap_or("  ");
    let entry = format!(
        "{}: {}",
        serde_json::to_string(key).expect("json"),
        render(value, unit, &members)
    );
    match c.children.last() {
        None => format!(
            "{}\n{members}{entry}\n{base}{}",
            &text[..=open],
            &text[c.close..]
        ),
        Some(last) => format!(
            "{},\n{members}{entry}{}",
            &text[..last.end],
            &text[last.end..]
        ),
    }
}

/// Adds `item` as the last item of the array at `open`.
pub fn add_item(text: &str, open: usize, item: &Value) -> String {
    let c = container(text.as_bytes(), open);
    let Some(last) = c.children.last() else {
        let item = serde_json::to_string(item).expect("json");
        return format!("{}{item}{}", &text[..=open], &text[c.close..]);
    };
    let first = &c.children[0];
    let multi = text[open..first.at].contains('\n');
    let (sep, item) = match line_indent(text, first.at) {
        Some(i) if multi => {
            let unit = i
                .strip_prefix(base_indent(text, open))
                .filter(|u| !u.is_empty())
                .unwrap_or("  ");
            (format!(",\n{i}"), render(item, unit, i))
        }
        _ => (", ".to_string(), serde_json::to_string(item).expect("json")),
    };
    format!("{}{sep}{item}{}", &text[..last.end], &text[last.end..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn member_keeps_order_and_indent() {
        let text = "{\n    \"z\": 1,\n    \"a\": [1, 2]\n}\n";
        let out = add_member(text, root(text), "m", &json!({"k": true}));
        assert_eq!(
            out,
            "{\n    \"z\": 1,\n    \"a\": [1, 2],\n    \"m\": {\n        \"k\": true\n    }\n}\n"
        );
    }

    #[test]
    fn member_of_empty_object() {
        let text = "{}\n";
        assert_eq!(
            add_member(text, root(text), "a", &json!(1)),
            "{\n  \"a\": 1\n}\n"
        );
    }

    #[test]
    fn items_keep_the_line_style() {
        let multi = "{\n\t\"r\": [\n\t\t\"a\"\n\t]\n}";
        let at = member(multi, root(multi), "r").unwrap();
        assert_eq!(
            add_item(multi, at, &json!("b")),
            "{\n\t\"r\": [\n\t\t\"a\",\n\t\t\"b\"\n\t]\n}"
        );
        let one = "{\"r\": [\"a\"]}";
        let at = member(one, root(one), "r").unwrap();
        assert_eq!(add_item(one, at, &json!("b")), "{\"r\": [\"a\", \"b\"]}");
        let none = "{\"r\": []}";
        let at = member(none, root(none), "r").unwrap();
        assert_eq!(add_item(none, at, &json!("b")), "{\"r\": [\"b\"]}");
    }

    #[test]
    fn an_object_item_of_a_multiline_array_is_indented_like_it() {
        let text = "{\n  \"r\": [\n    1\n  ]\n}";
        let at = member(text, root(text), "r").unwrap();
        assert_eq!(
            add_item(text, at, &json!({"k": 2})),
            "{\n  \"r\": [\n    1,\n    {\n      \"k\": 2\n    }\n  ]\n}"
        );
    }

    #[test]
    fn strings_with_brackets_do_not_confuse_the_scan() {
        let text = "{\"a\": \"}]\\\"\", \"b\": 1}";
        assert!(member(text, root(text), "b").is_some());
    }
}
