use std::{fs, path::PathBuf};

use roxmltree::{Document, Node};

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn load_valid(name: &str) -> (String, String) {
    let root = fixtures_root().join("valid");
    let txt = fs::read_to_string(root.join(format!("{name}.txt")))
        .unwrap_or_else(|e| panic!("missing valid/{name}.txt: {e}"));
    let xml = fs::read_to_string(root.join(format!("{name}.xml")))
        .unwrap_or_else(|e| panic!("missing valid/{name}.xml: {e}"));
    (txt, xml)
}

pub fn load_invalid(name: &str) -> String {
    let path = fixtures_root().join("invalid").join(format!("{name}.txt"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("missing {path:?}: {e}"))
}

pub fn assert_xml_equal(actual: &str, expected: &str) {
    let a_doc = Document::parse(actual).expect("actual XML failed to parse");
    let e_doc = Document::parse(expected).expect("expected XML failed to parse");
    compare(a_doc.root_element(), e_doc.root_element(), String::new());
}

fn compare(a: Node, b: Node, path: String) {
    let here = format!("{path}/{}", a.tag_name().name());
    assert_eq!(
        a.tag_name().name(),
        b.tag_name().name(),
        "tag mismatch at {path}: {} vs {}",
        a.tag_name().name(),
        b.tag_name().name()
    );

    let a_text = normalized_text(a);
    let b_text = normalized_text(b);
    assert_eq!(
        a_text, b_text,
        "text mismatch at {here}: {a_text:?} vs {b_text:?}"
    );

    let a_children: Vec<Node> = a.children().filter(Node::is_element).collect();
    let b_children: Vec<Node> = b.children().filter(Node::is_element).collect();
    assert_eq!(
        a_children.len(),
        b_children.len(),
        "child count mismatch at {here}: actual={:?} expected={:?}",
        a_children
            .iter()
            .map(|n| n.tag_name().name())
            .collect::<Vec<_>>(),
        b_children
            .iter()
            .map(|n| n.tag_name().name())
            .collect::<Vec<_>>(),
    );

    for (ac, bc) in a_children.iter().zip(b_children.iter()) {
        compare(*ac, *bc, here.clone());
    }
}

fn normalized_text(n: Node) -> String {
    let direct: String = n
        .children()
        .filter(|c| c.is_text())
        .filter_map(|c| c.text())
        .collect();
    direct.trim().to_string()
}
