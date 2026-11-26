// The go-template parser (scripts/evidence_helm_parsers/fetch-grammar.sh) is compiled for the `ts` binary only.
fn main() {
    if std::env::var_os("CARGO_FEATURE_TS").is_none() {
        return;
    }
    let src = std::path::Path::new("gotmpl/src");
    println!("cargo:rerun-if-changed=gotmpl/src/parser.c");
    cc::Build::new()
        .include(src)
        .file(src.join("parser.c"))
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs")
        .compile("gotmpl");
}
