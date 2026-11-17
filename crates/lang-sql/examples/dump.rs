//! Temporary: compact tree dump. `dump <dialect> <file> [new]`; one-line S-expression.
use core_syntax::SyntaxElement;
use provider_api::LanguageProvider;
fn go(e: SyntaxElement, g: &core_syntax::Grammar, out: &mut String) {
    match e {
        SyntaxElement::Node(n) => {
            out.push_str(&format!("({}", g.name(n.kind())));
            for c in n.children_with_tokens() {
                go(c, g, out);
            }
            out.push(')');
        }
        SyntaxElement::Token(t) => {
            if g.is_trivia(t.kind()) {
                return;
            }
            let name = g.name(t.kind());
            let short = match name {
                "Keyword" => "k",
                "Identifier" => "id",
                "FunctionIdentifier" => "fn",
                "TypeIdentifier" => "ty",
                "WordLiteral" => "lit",
                "BinaryKeyword" => "bk",
                "BinaryOperator" => "bo",
                "Operator" => "op",
                "Number" => "n",
                "String" => "s",
                "QuotedIdentifier" => "qi",
                "Word" => "w",
                "Indent" => "I",
                "ImplicitIndent" => "i",
                "Dedent" => "D",
                "Eof" => "EOF",
                "Unknown" => "UNK",
                _ => "",
            };
            match short {
                "I" | "i" | "D" => out.push_str(&format!(" <{short}>")),
                "" => out.push_str(&format!(" {}/{}", t.text(), name)),
                _ => out.push_str(&format!(" {}/{short}", t.text())),
            }
        }
    }
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let text = if a[2] == "-e" { a[3].clone() } else { std::fs::read_to_string(&a[2]).unwrap() };
    let r = if a.len() > 4 || (a.len() > 3 && a[2] != "-e") { lang_sql::parse_hand(&text, &a[1]) } else { lang_sql::parse(&text, &a[1]) };
    let g = lang_sql::provider().grammar().clone();
    let mut out = String::new();
    go(SyntaxElement::Node(r.tree.clone()), &g, &mut out);
    println!("{out}");
    for e in &r.errors {
        println!("ERR {:?}", e);
    }
}
