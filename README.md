# txt → XML parser

Readme written by CLAUDE with my guidance

Solution to the Softhouse Norr code test (see `tu-norr.pdf`). Converts a line-based
pipe-separated record format into a nested XML structure.

Written in Rust.

## Prerequisites

Rust toolchain (tested with 1.90, anything reasonably modern should work).
Install via [rustup](https://rustup.rs).

## Running

```sh
cargo run                  # reads input.txt, prints XML to stdout
cargo run -- some.txt      # reads given file
cat some.txt | cargo run -- -    # reads stdin
```

XML goes to **stdout**. Any parse issues (malformed rows, schema violations, etc.)
go to **stderr** as a report after the XML. The process exits with code `1` if any
issues were collected, `0` otherwise.

### Example

Running against the bundled `input.txt` (the PDF's canonical example) produces:

```
$ cargo run --quiet
<people>
  <person>
    <firstname>Carl Gustaf</firstname>
    <lastname>Bernadotte</lastname>
    <phone>
      <mobile>0768-101801</mobile>
      <landline>08-101801</landline>
    </phone>
    <address>
      <street>Drottningholms slott</street>
      <city>Stockholm</city>
      <zip>10001</zip>
    </address>
    <family>
      <name>Victoria</name>
      <born>1977</born>
      <address>
        <street>Haga Slott</street>
        <city>Stockholm</city>
        <zip>10002</zip>
      </address>
    </family>
    <family>
      <name>Carl Philip</name>
      <born>1979</born>
      <phone>
        <mobile>0768-101802</mobile>
        <landline>08-101802</landline>
      </phone>
    </family>
  </person>
  <person>
    <firstname>Barack</firstname>
    <lastname>Obama</lastname>
    <address>
      <street>1600 Pennsylvania Avenue</street>
      <city>Washington, D.C</city>
      <zip></zip>
    </address>
  </person>
</people>
======== Parsing Error Report ========
A expects 3 fields, got 2
line 9: A|1600 Pennsylvania Avenue|Washington, D.C
```

Obama's address (`A|street|city` without the postal code) violates the strict
3-field rule for `A`. Rather than dropping the entire row, the parser salvages
the provided fields, emits an empty `<zip></zip>` for the missing one, and
records the violation in the report. Exit code is `1` because an issue was
collected.

## Testing

```sh
cargo test
```

## Design philosophy

The parser is designed for the consultancy migration scenario described in the
task: an old system delivers a record-based file that contains real-world messiness
(missing optional fields, malformed rows, unknown row types, etc.). Rather than
bailing out on the first problem, the parser **tries to salvage as much valid data
as possible** and returns an issue report alongside the produced tree.

Concretely:

- `parse(text)` returns `(Tree, ParseReport)`. It never panics, never returns a
  fatal `Err`. The tree contains whatever could be salvaged; the report contains
  one `ParseError` per bad row.
- The parser uses a **peek-then-commit** algorithm: when a row arrives, it scans
  the stack of currently-open containers for one that accepts the row's tag. If
  none accepts, the row is dropped and a `ParseError` is pushed to the report —
  but the stack stays intact, so subsequent rows still attach correctly.
- The grammar is encoded as a small allow-list table (`SPECS`). Each row type
  declares its tag, the XML element name it produces, its field names, the
  minimum required field count, and which child tags it accepts. Adding a new
  row type is a one-line addition to the table.
- Field-count rules are **per-spec**: `P`, `F` require exactly 2 fields; `A`
  requires exactly 3 (street/city/zip all mandatory per the PDF spec); `T`
  accepts 1 or 2 fields (mobile required, landline optional — matches how phone
  data is typically captured today).
- When a row's field count is **out of range**, the parser **does not drop the
  row**. Too many fields → it keeps the first N (where N is the spec's max) and
  drops the extras. Too few → it pads the missing trailing fields with empty
  values. Either way it emits the row and records a warning in the report.
  Migration philosophy: keep the data, surface the violation.

## Tests

The test suite is split between fixture-driven cases (text/XML file pairs under
`tests/fixtures/`) and hand-written behavior tests in `tests/parser_tests.rs`.

> **Disclosure:** the test suite (fixtures and hand-written behavior tests) was
> generated with Claude (an AI assistant) to maximize coverage across happy paths,
> edge cases, malformed input, and recovery invariants. Claude also performed a
> couple of code-audit passes that surfaced bugs — notably the missing XML
> escaping for `&` and `<` characters.

### Valid fixtures (11)

Each runs through the parser and is asserted to produce the matching `.xml` output
with **zero** issues in the report.

| Fixture | Covers |
|---|---|
| `single_person` | Minimal happy path — a `P` row with no children |
| `person_with_family` | `P` containing one `F` containing its own `T` and `A` |
| `multi_person` | Multiple `P` blocks with full subtrees |
| `empty_fields` | Records with empty pipe-separated values (e.g. `T||08-1234`) |
| `empty_input` | An empty file produces `<people/>` |
| `whitespace_only_input` | A file containing only blank/whitespace lines also produces `<people/>` |
| `crlf_endings` | Windows-style `\r\n` line endings are handled |
| `xml_specials` | `&`, `<`, `>` in field values are XML-escaped |
| `repeated_records` | Multiple `T` and multiple `A` under one `P` render in document order |
| `multi_families` | Three sibling `F` blocks under one `P` — exercises F-closes-prior-F |
| `mobile_only_phone` | `T\|0700-123456` with no landline field — exercises optional trailing field |

### Invalid fixtures (6)

Each is asserted to produce **exactly one** issue at a known line number.

| Fixture | Cause | Expected line |
|---|---|---|
| `f_before_p` | `F` row with no enclosing `P` | 1 |
| `orphan_t` | `T` row before any `P` | 1 |
| `unknown_tag` | A row with a tag char other than `P/T/A/F` | 2 |
| `wrong_field_count` | `P` with too few fields | 1 |
| `too_many_fields` | `P` with too many fields | 1 |
| `malformed_no_pipe` | A row with no `|` separator | 1 |

### Behavior tests (7)

Hand-written tests asserting specific invariants of the parser's recovery model.

| Test | Asserts |
|---|---|
| `pdf_example_recovers_with_obama_issue` | The PDF's canonical example parses, with exactly one issue at line 9 (Obama's address is missing the postal code, which the schema requires). |
| `orphan_leaf_does_not_break_following_records` | An orphan `T` at line 1 produces an issue but does not corrupt the parser state — a subsequent valid `P|T` pair parses cleanly. |
| `bad_row_inside_open_family_keeps_family_open` | A malformed row appearing while an `F` is open does not close the family — the next `T` still attaches to the family. |
| `multiple_bad_rows_in_a_row_preserve_open_family` | Even three bad rows in a row do not close the open container; the next valid leaf still attaches correctly. |
| `multiple_issues_are_all_collected_with_correct_line_nos` | A file with four different errors at four different lines produces four issues with the correct line numbers — no bail-out. |
| `new_p_closes_open_family_via_walk_up` | A new `P` row closes any open `F` and the previous `P`, even when the `F` is the current attach target. |
| `new_f_closes_previous_f` | Two sibling `F` blocks under one `P` are correctly separated — the second F's children do not leak into the first. |
