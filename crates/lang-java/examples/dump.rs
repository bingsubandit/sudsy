use core_syntax::{dump_tree, grammars};
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let src = std::fs::read_to_string(path).unwrap();
    let r = lang_java::parse(&src);
    print!("{}", dump_tree(&r.tree, grammars::JAVA.get()));
}
