//! scratch (not committed): DUMP=file cargo test -p lang-js dump -- --nocapture
#[test]
fn dump() {
    let Ok(path) = std::env::var("DUMP") else { return };
    let text = std::fs::read_to_string(&path).unwrap();
    use provider_api::LanguageProvider;
    let file = provider_api::SourceFile::new(path.clone(), 1, text);
    let r = lang_js::provider().parse(&file).unwrap();
    print!("{}", core_syntax::dump_tree(&r.tree, core_syntax::grammars::js()));
}
