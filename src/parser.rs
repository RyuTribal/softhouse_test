use std::fmt;

// Claude found this issue.
// Checked the specs for xml
// The spec mandates & and < escaping with "MUST" (RFC 2119 — non-negotiable).
// >, ', " predefined entities exist but are optional in text content
// (required only in specific contexts: ]]> sequence, attribute value matching
// the quote char).
// Taken from sections 2.4 and 4.6 in w3.org/TR/xml.
fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
pub struct Spec {
    pub tag_in: &'static str,
    pub tag_out: &'static str,
    pub field_names: &'static [&'static str],
    pub min_fields: usize,
    pub allowed_children: &'static [&'static str],
}

impl Spec {
    pub fn is_container(&self) -> bool {
        !self.allowed_children.is_empty()
    }

    pub fn accepts(&self, child_tag_in: &str) -> bool {
        self.allowed_children.contains(&child_tag_in)
    }
}

pub const ROOT_SPEC: Spec = Spec {
    tag_in: "",
    tag_out: "people",
    field_names: &[],
    min_fields: 0,
    allowed_children: &["P"],
};

pub const SPECS: &[Spec] = &[
    Spec {
        tag_in: "P",
        tag_out: "person",
        field_names: &["firstname", "lastname"],
        min_fields: 2,
        allowed_children: &["T", "A", "F"],
    },
    Spec {
        tag_in: "F",
        tag_out: "family",
        field_names: &["name", "born"],
        min_fields: 2,
        allowed_children: &["T", "A"],
    },
    Spec {
        tag_in: "T",
        tag_out: "phone",
        field_names: &["mobile", "landline"],
        min_fields: 1,
        allowed_children: &[],
    },
    Spec {
        tag_in: "A",
        tag_out: "address",
        field_names: &["street", "city", "zip"],
        min_fields: 3,
        allowed_children: &[],
    },
];

#[derive(Debug, Clone)]
pub struct Node {
    pub tag: &'static str,
    pub text: Option<String>,
    pub children: Vec<usize>,
    pub spec: Option<&'static Spec>,
}

#[derive(Debug, Clone)]
pub struct Tree {
    pub nodes: Vec<Node>,
}

impl Tree {
    pub fn render(&self) -> String {
        let mut out = String::new();
        self.write(0, &mut out, 0);
        out
    }

    fn write(&self, idx: usize, out: &mut String, depth: usize) {
        let node = &self.nodes[idx];
        let indent = "  ".repeat(depth);

        if node.children.is_empty() && node.text.is_none() {
            out.push_str(&format!("{indent}<{}/>\n", node.tag));
            return;
        }

        if let Some(text) = &node.text {
            out.push_str(&format!(
                "{indent}<{0}>{1}</{0}>\n",
                node.tag,
                escape_xml_text(text)
            ));
            return;
        }

        out.push_str(&format!("{indent}<{}>\n", node.tag));
        for &c in &node.children {
            self.write(c, out, depth + 1);
        }
        out.push_str(&format!("{indent}</{}>\n", node.tag));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line_no: usize,
    pub line_context: String,
    pub message: String,
}

impl ParseError {
    pub fn new(
        line_no: usize,
        line_context: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            line_no,
            line_context: line_context.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\nline {}: {}\n",
            self.message, self.line_no, self.line_context
        )
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone)]
pub struct ParseReport {
    pub issues: Vec<ParseError>,
}

pub struct TreeBuilder {
    nodes: Vec<Node>,
    stack: Vec<usize>,
    issues: Vec<ParseError>,
}

impl TreeBuilder {
    pub fn new() -> Self {
        Self {
            nodes: vec![Node {
                tag: ROOT_SPEC.tag_out,
                text: None,
                children: vec![],
                spec: Some(&ROOT_SPEC),
            }],
            stack: vec![0],
            issues: vec![],
        }
    }

    pub fn ingest_line(&mut self, raw: &str, line_no: usize) {
        if raw.trim().is_empty() {
            return;
        }
        let (spec, mut values) = match Self::parse_line(raw, line_no) {
            Err(e) => {
                self.issues.push(e);
                return;
            }
            Ok(x) => x,
        };

        // Salvage out-of-range field counts:
        //   too many → take first max, drop extras, flag
        //   too few  → pad missing trailing fields with empty strings, flag
        let max = spec.field_names.len();
        let got = values.len();
        if got > max {
            values.truncate(max);
            self.issues.push(Self::range_issue(spec, got, line_no, raw));
        } else if got < spec.min_fields {
            while values.len() < max {
                values.push("");
            }
            self.issues.push(Self::range_issue(spec, got, line_no, raw));
        }

        if let Err(e) = self.apply(spec, &values, line_no, raw) {
            self.issues.push(e);
        }
    }

    fn range_issue(spec: &Spec, got: usize, line_no: usize, raw: &str) -> ParseError {
        let max = spec.field_names.len();
        let range = if spec.min_fields == max {
            format!("{max}")
        } else {
            format!("{}..={}", spec.min_fields, max)
        };
        ParseError::new(
            line_no,
            raw,
            format!("{} expects {range} fields, got {got}", spec.tag_in),
        )
    }

    fn parse_line(raw: &str, line_no: usize) -> Result<(&'static Spec, Vec<&str>), ParseError> {
        if !raw.contains('|') {
            return Err(ParseError::new(line_no, raw, "malformed row".to_string()));
        }

        let mut parts = raw.split('|');
        let tag = parts.next().unwrap_or("");
        let values: Vec<&str> = parts.collect();

        // Tbh an option here could be to blindly accept data. But dunno if thats smart.
        // Better to just flag I think since an unknown field is not part of the spec
        // Programmer rule nr 1 blame the specs not yourself
        let spec = lookup_spec(tag)
            .ok_or_else(|| ParseError::new(line_no, raw, format!("unknown tag {tag:?}")))?;

        Ok((spec, values))
    }

    fn apply(
        &mut self,
        spec: &'static Spec,
        values: &[&str],
        line_no: usize,
        raw: &str,
    ) -> Result<(), ParseError> {
        // Peek: scan stack from top, find deepest container that accepts spec.tag_in.
        // No mutation yet — if nothing accepts, the stack stays intact for later lines.
        let acceptor = self.stack.iter().rposition(|&idx| {
            self.nodes[idx]
                .spec
                .map(|s| s.accepts(spec.tag_in))
                .unwrap_or(false)
        });

        let pos = acceptor.ok_or_else(|| {
            ParseError::new(
                line_no,
                raw,
                format!(
                    "{} cannot be attached here (no enclosing container accepts it)",
                    spec.tag_in
                ),
            )
        })?;

        // Commit: close any containers above the acceptor.
        self.stack.truncate(pos + 1);
        let parent = self.stack[pos];
        let new_idx = self.push_with_fields(parent, spec, values);
        if spec.is_container() {
            self.stack.push(new_idx);
        }
        Ok(())
    }

    fn push_with_fields(&mut self, parent: usize, spec: &'static Spec, values: &[&str]) -> usize {
        let new_idx = self.push(
            parent,
            Node {
                tag: spec.tag_out,
                text: None,
                children: vec![],
                spec: Some(spec),
            },
        );
        for (fname, fval) in spec.field_names.iter().zip(values.iter()) {
            self.push(
                new_idx,
                Node {
                    tag: fname,
                    text: Some((*fval).to_string()),
                    children: vec![],
                    spec: None,
                },
            );
        }
        new_idx
    }

    fn push(&mut self, parent: usize, node: Node) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        self.nodes[parent].children.push(idx);
        idx
    }

    pub fn finish(self) -> (Tree, ParseReport) {
        (
            Tree { nodes: self.nodes },
            ParseReport {
                issues: self.issues,
            },
        )
    }
}

impl Default for TreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn lookup_spec(tag: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.tag_in == tag)
}

pub fn parse(text: &str) -> (Tree, ParseReport) {
    let mut b = TreeBuilder::new();
    for (i, raw) in text.lines().enumerate() {
        b.ingest_line(raw, i + 1);
    }
    b.finish()
}
