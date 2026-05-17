mod common;

use txt_to_xml_parser::parse;

macro_rules! valid_case {
    ($name:ident) => {
        #[test]
        fn $name() {
            let (txt, expected) = common::load_valid(stringify!($name));
            let (tree, report) = parse(&txt);
            assert!(
                report.issues.is_empty(),
                "expected no issues, got {:?}",
                report.issues
            );
            common::assert_xml_equal(&tree.render(), &expected);
        }
    };
}

macro_rules! invalid_case {
    ($name:ident, $line:expr) => {
        #[test]
        fn $name() {
            let txt = common::load_invalid(stringify!($name));
            let (_tree, report) = parse(&txt);
            assert_eq!(
                report.issues.len(),
                1,
                "expected exactly 1 issue, got {}: {:?}",
                report.issues.len(),
                report.issues
            );
            assert_eq!(
                report.issues[0].line_no, $line,
                "issue at wrong line: {:?}",
                report.issues[0]
            );
        }
    };
}

mod valid {
    use super::*;

    valid_case!(single_person);
    valid_case!(person_with_family);
    valid_case!(multi_person);
    valid_case!(empty_fields);
    valid_case!(empty_input);
    valid_case!(whitespace_only_input);
    valid_case!(crlf_endings);
    valid_case!(xml_specials);
    valid_case!(repeated_records);
    valid_case!(multi_families);
    valid_case!(mobile_only_phone);
}

mod invalid {
    use super::*;

    invalid_case!(f_before_p, 1);
    invalid_case!(orphan_t, 1);
    invalid_case!(unknown_tag, 2);
    invalid_case!(wrong_field_count, 1);
    invalid_case!(too_many_fields, 1);
    invalid_case!(malformed_no_pipe, 1);
}

// Peek-then-commit: an issue must not corrupt the stack for subsequent rows.
#[test]
fn orphan_leaf_does_not_break_following_records() {
    let input = "\
T|orphan|phone
P|Alice|Smith
T|111|222
";
    let (tree, report) = parse(input);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].line_no, 1);

    let xml = tree.render();
    assert!(xml.contains("<firstname>Alice</firstname>"));
    assert!(xml.contains("<mobile>111</mobile>"));
    assert!(
        !xml.contains("orphan"),
        "orphan phone must not appear in output"
    );
}

#[test]
fn multiple_bad_rows_in_a_row_preserve_open_family() {
    let input = "\
P|Alice|Smith
F|Kid|2010
malformed one
also bad with no pipe
X|unknown|tag
T|999|888
";
    let (tree, report) = parse(input);
    assert_eq!(report.issues.len(), 3);
    assert_eq!(report.issues[0].line_no, 3);
    assert_eq!(report.issues[1].line_no, 4);
    assert_eq!(report.issues[2].line_no, 5);

    let xml = tree.render();
    let family_start = xml.find("<family>").expect("family present");
    let family_end = xml.find("</family>").expect("family closed");
    let family_block = &xml[family_start..family_end];
    assert!(
        family_block.contains("<mobile>999</mobile>"),
        "phone should still attach to Kid's family after 3 bad rows, got family:\n{family_block}"
    );
}

#[test]
fn bad_row_inside_open_family_keeps_family_open() {
    let input = "\
P|Alice|Smith
F|Kid|2010
malformed line with no separator
T|999|888
";
    let (tree, report) = parse(input);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].line_no, 3);

    // After the malformed line, Kid (the open family) should still be the
    // attach target, so the phone goes under family, not directly under person.
    let xml = tree.render();
    let family_start = xml.find("<family>").expect("family present");
    let family_end = xml.find("</family>").expect("family closed");
    let family_block = &xml[family_start..family_end];
    assert!(
        family_block.contains("<mobile>999</mobile>"),
        "phone should be inside Kid's family block, got family:\n{family_block}"
    );
}

#[test]
fn multiple_issues_are_all_collected_with_correct_line_nos() {
    let input = "\
F|EarlyFamily|2000
not a row at all
X|unknown|tag
P|OnlyOne
P|Real|Person
T|valid|phone
";
    let (_tree, report) = parse(input);
    assert_eq!(
        report.issues.len(),
        4,
        "expected 4 issues, got {:?}",
        report.issues
    );
    assert_eq!(report.issues[0].line_no, 1, "F before any P");
    assert_eq!(report.issues[1].line_no, 2, "malformed");
    assert_eq!(report.issues[2].line_no, 3, "unknown tag X");
    assert_eq!(report.issues[3].line_no, 4, "P with wrong field count");
}

#[test]
fn new_p_closes_open_family_via_walk_up() {
    let input = "\
P|Alice|Smith
F|Kid|2010
T|kid-phone|kid-landline
P|Bob|Jones
A|bob street|bob city|99999
";
    let (tree, report) = parse(input);
    assert!(
        report.issues.is_empty(),
        "no issues expected, got {:?}",
        report.issues
    );

    let xml = tree.render();
    // Bob's address should be under Bob, not under Kid's family.
    let bob_start = xml.find("<firstname>Bob</firstname>").expect("Bob present");
    let bob_block = &xml[bob_start..];
    assert!(
        bob_block.contains("<street>bob street</street>"),
        "Bob should own the second address"
    );
    // Kid's phone should be inside Kid's family.
    let kid_start = xml.find("<name>Kid</name>").expect("Kid present");
    let kid_block = &xml[kid_start..];
    assert!(
        kid_block.contains("<mobile>kid-phone</mobile>"),
        "Kid family should own the phone"
    );
}

#[test]
fn new_f_closes_previous_f() {
    let input = "\
P|Parent|X
F|First|2000
T|first-phone|first-landline
F|Second|2005
T|second-phone|second-landline
";
    let (tree, report) = parse(input);
    assert!(report.issues.is_empty());

    let xml = tree.render();
    let first_start = xml.find("<name>First</name>").expect("First present");
    let second_start = xml.find("<name>Second</name>").expect("Second present");
    assert!(first_start < second_start);

    let first_block = &xml[first_start..second_start];
    let second_block = &xml[second_start..];
    assert!(first_block.contains("<mobile>first-phone</mobile>"));
    assert!(second_block.contains("<mobile>second-phone</mobile>"));
    // First family must not contain second's phone (would mean F didn't close).
    assert!(!first_block.contains("second-phone"));
}

#[test]
fn pdf_example_recovers_with_obama_issue() {
    let (txt, expected) = common::load_valid("pdf_example");
    let (tree, report) = parse(&txt);
    common::assert_xml_equal(&tree.render(), &expected);
    assert_eq!(
        report.issues.len(),
        1,
        "expected exactly 1 issue: Obama's incomplete address"
    );
    assert_eq!(report.issues[0].line_no, 9, "Obama address is on line 9");
}
