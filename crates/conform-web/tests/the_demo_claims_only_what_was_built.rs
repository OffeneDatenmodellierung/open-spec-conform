//! The page cannot say it validates documents unless a module is there.
//!
//! # The failure this file exists to prevent
//!
//! A validation box that appears to work and does not. It is the worst
//! outcome available in this section by a distance — worse than no demo, and
//! much worse than a sentence explaining why there is none — because a reader
//! who pastes a contract into a dead box and sees nothing has been told
//! something false about their document by a project whose entire subject is
//! not telling people false things about their documents.
//!
//! [`Demo`] is a typed value so that the claim is a *measurement*: the
//! generator stats the artefacts and the variant follows from what it found.
//! This file holds that to more than an intention, by building directories
//! that do and do not contain a module and rendering the page from each.
//!
//! # And one thing the page must not write
//!
//! The standards on offer. They are the spec ids the module reports carrying a
//! verified schema for, added to the `select` by the browser at run time. A
//! list written into the HTML would be a second copy of a fact the artefact
//! already holds — which is the drift this whole repository is built around,
//! arriving in the one place where being wrong means offering a reader a
//! standard the validator beside them cannot check.

use std::fs;
use std::path::{Path, PathBuf};

use conform_registry::Registry;
use conform_web::{Demo, Module, Site, render};

/// A registry with one entry, so a page can be rendered at all.
///
/// The values are nonsense on purpose: nothing in this file is about the
/// catalogue, and a fixture that used a real specification's name would be a
/// specification fact written into this crate's tests — which
/// `no_spec_facts_are_written_in_the_source.rs` scans for and would catch.
fn registry() -> Registry {
    Registry::load_str(
        "schema_version = 1\n\
         [[spec]]\n\
         id = \"qqq\"\n\
         name = \"Quux Interchange Format\"\n\
         vendored_path = \"schemas/quux.json\"\n\
         sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n\
         fetched_at = \"2020-01-02\"\n",
        "<fixture>",
    )
    .expect("the fixture is a registry")
}

/// A directory nothing else in this run will touch.
///
/// Hand-rolled rather than a `tempfile` dependency: this crate takes no
/// dev-dependency it does not already have in the graph, and what is wanted
/// here is one directory under the target directory with a name nobody else
/// will pick.
fn scratch(name: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("a scratch directory");
    directory
}

/// Write the two files `Demo::look_for` requires, with the given contents.
fn plant(directory: &Path, script: &[u8], wasm: &[u8]) {
    fs::write(directory.join("conform_ffi.js"), script).expect("write the glue");
    fs::write(directory.join("conform_ffi_bg.wasm"), wasm).expect("write the module");
}

fn page_for(demo: Demo) -> String {
    let registry = registry();
    let site = Site::from_parts(&registry, Vec::new(), "fixture.toml".to_owned()).with_demo(demo);
    render::page(&site)
}

#[test]
fn an_empty_directory_is_not_a_demo() {
    let directory = scratch("demo-empty");
    let demo = Demo::look_for(&directory, "wasm");

    let Demo::NotWired { because, evidence } = &demo else {
        panic!("an empty directory produced {demo:?}");
    };
    assert!(
        because.contains("produced no WebAssembly module"),
        "the reason must be the build's, not a limitation: {because}"
    );
    // Both files are named, so a reader is not left guessing which is missing.
    assert_eq!(evidence.len(), 2, "{evidence:?}");
    assert!(
        evidence
            .iter()
            .any(|(what, _)| what.contains("conform_ffi.js"))
    );
    assert!(
        evidence
            .iter()
            .any(|(what, _)| what.contains("conform_ffi_bg.wasm"))
    );
}

#[test]
fn a_truncated_module_is_not_a_demo() {
    // The signature of an interrupted build. It has to fail here rather than
    // in somebody's browser, which is the whole reason the size is measured
    // instead of the existence being tested.
    let directory = scratch("demo-truncated");
    plant(&directory, b"export default function init() {}", b"");

    let demo = Demo::look_for(&directory, "wasm");
    let Demo::NotWired { evidence, .. } = &demo else {
        panic!("a zero-byte module produced {demo:?}");
    };
    assert!(
        evidence
            .iter()
            .any(|(_, result)| result.contains("interrupted build")),
        "{evidence:?}"
    );
}

#[test]
fn both_artefacts_present_is_a_demo_and_the_sizes_are_measured() {
    let directory = scratch("demo-present");
    plant(&directory, b"0123456789", b"0123456789abcde");

    let demo = Demo::look_for(&directory, "assets/wasm");
    let Demo::Wired(module) = &demo else {
        panic!("a complete directory produced {demo:?}");
    };
    assert_eq!(module.script_href, "./assets/wasm/conform_ffi.js");
    assert_eq!(module.wasm_href, "./assets/wasm/conform_ffi_bg.wasm");
    assert_eq!(module.script_bytes, 10);
    assert_eq!(module.wasm_bytes, 15);
}

#[test]
fn a_page_without_a_module_has_no_validation_box() {
    let page = page_for(Demo::look_for(&scratch("demo-none"), "wasm"));

    for element in [
        "id=\"demo-document\"",
        "id=\"demo-run\"",
        "id=\"demo-spec\"",
        "<script type=\"module\">",
    ] {
        assert!(
            !page.contains(element),
            "a page with no module still wrote {element}"
        );
    }
    assert!(
        page.contains("Not in this build."),
        "a page with no module must say so"
    );
}

#[test]
fn a_page_with_a_module_loads_it_from_an_attribute_and_not_from_a_script_literal() {
    let page = page_for(Demo::Wired(Module {
        script_href: "./wasm/conform_ffi.js".to_owned(),
        script_bytes: 19_322,
        wasm_href: "./wasm/conform_ffi_bg.wasm".to_owned(),
        wasm_bytes: 3_112_486,
    }));

    // The one value the script needs reaches it through a `data-` attribute,
    // which went through the page's single escaping boundary. Nothing this
    // generator produced is interpolated into JavaScript, because the moment
    // one thing is, the rule stops being checkable by reading the constants.
    assert!(page.contains("data-module=\"./wasm/conform_ffi.js\""));
    assert!(page.contains("panel.dataset.module"));
    assert!(
        !page.contains("import(\"./wasm/"),
        "the module path was written into the script instead of an attribute"
    );

    // The measured sizes are shown. A reader on a metered connection is owed
    // the number before the fetch starts, not after.
    assert!(
        page.contains("3112486"),
        "the module's size is not on the page"
    );
    assert!(page.contains("19322"), "the glue's size is not on the page");

    assert!(page.contains("id=\"demo-document\""));
    assert!(page.contains("<script type=\"module\">"));
}

/// The ways a script can hand a string to the HTML parser.
///
/// Written as *uses* — a property access, a call — rather than as bare words,
/// because both scripts on this page talk about `innerHTML` in comments
/// explaining why they do not use it, and a scan that could not tell the
/// prohibition from the offence would flag the thing that documents the rule.
/// [`the_inner_html_scan_can_see_a_real_one`] is the control for that.
const HANDS_A_STRING_TO_THE_PARSER: &[&str] = &[
    ".innerHTML",
    ".outerHTML",
    ".insertAdjacentHTML(",
    "document.write(",
];

#[test]
fn no_page_hands_a_string_to_the_html_parser() {
    // The rule the whole escaping design rests on, checked against the bytes
    // rather than against the renderer's intentions. A diagnostic message
    // quotes the reader's own document verbatim; one `innerHTML` in the demo
    // script turns this page into a self-XSS machine, and nothing that
    // inspects only the *generated* markup would catch it, because the
    // element would be built in the browser.
    for demo in [
        Demo::not_looked_for(),
        Demo::Wired(Module {
            script_href: "./wasm/conform_ffi.js".to_owned(),
            script_bytes: 1,
            wasm_href: "./wasm/conform_ffi_bg.wasm".to_owned(),
            wasm_bytes: 1,
        }),
    ] {
        let page = page_for(demo);
        for forbidden in HANDS_A_STRING_TO_THE_PARSER {
            assert!(
                !page.contains(forbidden),
                "`{forbidden}` appears on the page; every runtime value must be written with \
                 textContent"
            );
        }
        // Not vacuous: the page must actually be setting text at run time, or
        // the scan above is looking at a document with no script worth
        // scanning.
        assert!(
            page.contains(".textContent"),
            "the page sets nothing with textContent, so the scan proves nothing"
        );
    }
}

#[test]
fn the_inner_html_scan_can_see_a_real_one() {
    // The control. The needles have to fire on a genuine use and stay silent
    // on the comments that explain why there is none — which is exactly the
    // distinction an earlier, cruder version of this scan could not draw, and
    // it flagged the comment.
    let offence = "node.innerHTML = finding.message;";
    let prohibition = "// textContent, never innerHTML: see this constant's documentation.";

    assert!(
        HANDS_A_STRING_TO_THE_PARSER
            .iter()
            .any(|needle| offence.contains(needle)),
        "the scan cannot see a real use"
    );
    assert!(
        !HANDS_A_STRING_TO_THE_PARSER
            .iter()
            .any(|needle| prohibition.contains(needle)),
        "the scan flags the comment that forbids it"
    );
}

#[test]
fn the_page_never_writes_the_standards_the_module_offers() {
    // The `select` ships empty and is filled from `reachableSpecIds()`. A page
    // that listed them could offer a standard the artefact beside it cannot
    // validate — and would go on offering it after the module stopped
    // carrying that schema, with nothing to notice.
    let page = page_for(Demo::Wired(Module {
        script_href: "./wasm/conform_ffi.js".to_owned(),
        script_bytes: 1,
        wasm_href: "./wasm/conform_ffi_bg.wasm".to_owned(),
        wasm_bytes: 1,
    }));

    assert!(
        page.contains("<select id=\"demo-spec\"></select>"),
        "the standards list must be empty in the generated bytes"
    );
    assert!(
        page.contains("wasm.reachableSpecIds()"),
        "the standards must be read from the module"
    );
    assert!(
        page.contains("wasm.panicContract()"),
        "the sentence about what a panic does must be the module's own"
    );
}
