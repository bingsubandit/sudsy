// TEMPORARY probe (not committed).
use core_syntax::dump_tree;
use provider_api::{LanguageProvider, SourceFile};

fn provider_for(name: &str) -> provider_api::ParserProvider {
    if name.ends_with(".vue") {
        lang_html::vue_provider()
    } else if name.ends_with(".svelte") {
        lang_html::svelte_provider()
    } else if name.ends_with(".component.html") {
        lang_html::angular_provider()
    } else {
        lang_html::provider()
    }
}

#[test]
#[ignore]
fn probe() {
    let file = std::env::var("PROBE_FILE").unwrap();
    let name = std::env::var("PROBE_NAME").unwrap_or(file.clone());
    let text = std::fs::read_to_string(&file).unwrap();
    let p = provider_for(&name);
    let r = p.parse(&SourceFile::new(&name, 1, &text)).unwrap();
    println!("{}", dump_tree(&r.tree, p.grammar()));
    for e in &r.errors {
        println!("ERR {}..{} {}", e.start, e.start + e.len, e.message);
    }
}
