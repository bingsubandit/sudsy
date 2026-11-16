#[test]
#[ignore]
fn dump() {
    let src = std::fs::read_to_string(std::env::var("DUMP").unwrap()).unwrap();
    println!("{:#?}", lang_php::parse(&src).tree);
}
