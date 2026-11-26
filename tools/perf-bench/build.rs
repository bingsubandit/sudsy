//! The bench builds this harness against the baseline's crates too (scripts/bench.sh): `file_cells`
//! is set when `core_rules::FileContext` has its `cells` field, so the harness builds on both sides.
//! Delete once the baseline has the field.
fn main() {
    let rule = "../../crates/core-rules/src/rule.rs";
    println!("cargo::rerun-if-changed={rule}");
    println!("cargo::rustc-check-cfg=cfg(file_cells)");
    if std::fs::read_to_string(rule).is_ok_and(|s| s.contains("pub cells: &'a [TextRange]")) {
        println!("cargo::rustc-cfg=file_cells");
    }
}
