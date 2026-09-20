//! A field nobody could determine must read as unknown, in every rendering.
//!
//! The registry's central rule is that a field nobody could determine is left
//! out, and the entry's `notes` say what was looked at and what it did not
//! say — because a recorded unknown is usable evidence, and a
//! plausible-looking value nobody checked is worse than silence, since it
//! reads as verified.
//!
//! A renderer is the last place that rule can be broken, and it is broken by
//! accident rather than by intent: `unwrap_or_default()` on an `Option<String>`
//! is one keystroke, looks harmless, and quietly converts "nobody knows the
//! licence of these bytes" into "the licence is the empty string". This test
//! is what makes that keystroke fail.
//!
//! Three of the five entries in this repository's registry record no licence
//! and three record no homepage, so the corpus is real rather than contrived.

mod support;

use serde_json::Value;
use support::{conform, conform_json};

#[test]
fn an_unrecorded_field_is_json_null_and_never_an_empty_string() {
    let (json, code) = conform_json(&["registry", "list"]);
    assert_eq!(code, 0);

    let specs = json["specs"].as_array().expect("specs is an array");
    assert_eq!(specs.len(), 5, "the registry holds five entries");

    let mut nulls = 0usize;
    for spec in specs {
        let upstream = &spec["upstream"];
        for field in ["homepage", "repository", "pinned_ref", "steward", "licence"] {
            let value = &upstream[field];
            match value {
                Value::Null => nulls += 1,
                Value::String(text) => assert!(
                    !text.trim().is_empty(),
                    "`{}`.{field} is present and blank, which is neither an answer nor an \
                     honest absence",
                    spec["id"]
                ),
                other => panic!("`{}`.{field} is {other}, not a string or null", spec["id"]),
            }
            // The key is always emitted. An omitted key and a `null` are
            // different things to a consumer with a strict schema, and only
            // one of them says "we looked and there was nothing".
            assert!(
                upstream.get(field).is_some(),
                "`{}` omits the `{field}` key entirely",
                spec["id"]
            );
        }
    }

    assert!(
        nulls >= 6,
        "only {nulls} absences were found; this registry records more than that, so the \
         renderer is filling something in"
    );
}

#[test]
fn the_absences_json_reports_are_the_ones_the_registry_itself_names() {
    // The two must not be derived separately. `provenance_gaps` is the
    // registry's own list; the nulls are this renderer's. They agree here, or
    // one of them is wrong.
    let (json, _) = conform_json(&["registry", "list"]);

    for spec in json["specs"].as_array().expect("specs is an array") {
        let gaps: Vec<&str> = spec["provenance_gaps"]
            .as_array()
            .expect("provenance_gaps is an array")
            .iter()
            .map(|gap| gap.as_str().expect("a field name"))
            .collect();

        for field in ["homepage", "repository", "steward", "licence", "pinned_ref"] {
            assert_eq!(
                spec["upstream"][field].is_null(),
                gaps.contains(&field),
                "`{}`.{field}: the null and the registry's own gap list disagree",
                spec["id"]
            );
        }
    }
}

#[test]
fn the_human_catalogue_says_not_recorded_rather_than_leaving_a_blank() {
    let output = conform(&["registry", "list"]);
    assert_eq!(output.code, 0);

    // ODCL records no homepage, no steward and no licence — three absences in
    // one entry, and its notes explain each.
    assert!(output.stdout.contains("odcl"));
    assert!(output.stdout.contains("(not recorded)"));

    // A blank after a field label would look like a rendering accident rather
    // than an answer. Every labelled line must carry something after its label.
    let mut labelled = 0usize;
    for line in output.stdout.lines() {
        let content = line.trim();
        for field in ["homepage", "repository", "steward", "licence", "pinned_ref"] {
            let Some(rest) = content.strip_prefix(field) else {
                continue;
            };
            // `gaps         licence` names the field rather than labelling it,
            // and is the registry's own list. Only a line that *starts* with
            // the label is a value line.
            assert!(
                !rest.trim().is_empty(),
                "`{field}` was printed with nothing after it: {line:?}"
            );
            labelled += 1;
        }
    }
    assert!(
        labelled >= 25,
        "only {labelled} labelled provenance lines were seen across five entries"
    );
}

#[test]
fn every_entry_carries_the_upstream_link_and_the_pin_the_console_promises() {
    // The requirement the TUI's spec pane is built on: for any selected
    // specification, the upstream link, the pin and the drift status are
    // available without a second lookup. They come from here.
    let (json, _) = conform_json(&["registry", "verify"]);

    for spec in json["specs"].as_array().expect("specs is an array") {
        assert!(
            spec["upstream"]["link"].is_string(),
            "`{}` has no upstream link at all, and every entry in this registry records at \
             least a repository",
            spec["id"]
        );
        assert!(spec["upstream"]["pinned_ref"].is_string());
        assert_eq!(spec["vendored"]["verify"], "matched");
        assert_eq!(spec["vendored"]["verify_code"], "REG023");
    }
}
