use crate::generated::kind as k;
use provider_api::{SourceFile, guard};

/// A list that ends in a pipeline is a list of the pipeline, never a pipeline whose element is a
/// list: the layout puts the operators of a flow at the level of the flow they belong to.
#[test]
fn a_pipeline_never_holds_a_list() {
    let provider = crate::provider();
    let text = "a >b && f | c | s | x && r";
    let parsed = guard::parse(&provider, &SourceFile::new("a.sh", 1, text)).unwrap();
    let nodes: Vec<_> = parsed.tree.descendants().collect();
    assert!(nodes.iter().any(|n| n.kind() == k::PIPELINE));
    assert!(
        nodes
            .iter()
            .filter(|n| n.kind() == k::PIPELINE)
            .all(|p| p.children().all(|c| c.kind() != k::LIST)),
        "{}",
        parsed.tree.text()
    );
}
