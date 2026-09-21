//! Record what the pre-existing validators say about the adapter crates'
//! fixture corpora.
//!
//! This calls the real functions — `data_modelling_core::validation::schema::
//! validate_odcs_internal`, `validate_odcl_internal` and
//! `validate_odps_internal` — over every fixture,
//! and writes what they returned to a JSON file each adapter crate's
//! differential test reads back.
//!
//! It is a *recorder*, not the test. The test lives in
//! `crates/conform-*/tests/oracle_agreement.rs` and compares this crate's
//! verdict against the recording. Why the indirection is necessary, and what
//! it costs, is argued in `Cargo.toml` beside the dependency and in
//! `README.md` beside this file; the short version is that calling the oracle
//! from inside the workspace would put `yaml-rust 0.4.5` into the dependency
//! graph `cargo deny` gates on, and RUSTSEC-2024-0320 says there is no safe
//! upgrade. The recording carries a SHA-256 per fixture so it cannot silently
//! go stale when a fixture is edited.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// One corpus to record.
struct Corpus {
    /// The adapter crate the fixtures belong to.
    crate_dir: &'static str,
    /// Where the recording is written, relative to the repository root.
    output: &'static str,
    /// The oracle function, spelled as a consumer would call it.
    function: &'static str,
    /// The schema file the oracle `include_str!`s, relative to the SDK root.
    /// Recorded so the comparison can be shown to be against the same bytes.
    sdk_schema: &'static str,
    /// The vendored copy this repository validates against.
    vendored_schema: &'static str,
    /// The oracle itself.
    validate: fn(&str) -> Result<(), String>,
}

const CORPORA: &[Corpus] = &[
    Corpus {
        crate_dir: "crates/conform-odcs",
        output: "crates/conform-odcs/tests/oracle/odcs-verdicts.json",
        function: "data_modelling_core::validation::schema::validate_odcs_internal",
        sdk_schema: "schemas/odcs-json-schema-v3.1.0.json",
        vendored_schema: "schemas/odcs-json-schema-v3.1.0.json",
        validate: data_modelling_core::validation::schema::validate_odcs_internal,
    },
    Corpus {
        crate_dir: "crates/conform-odps",
        output: "crates/conform-odps/tests/oracle/odps-verdicts.json",
        function: "data_modelling_core::validation::schema::validate_odps_internal",
        // The SDK's copy is still the unpinned filename. It is byte-identical
        // to this repository's `odps-json-schema-v1.0.0.json`, and the digests
        // recorded below are what proves it rather than asserts it.
        sdk_schema: "schemas/odps-json-schema-latest.json",
        vendored_schema: "schemas/odps-json-schema-v1.0.0.json",
        validate: data_modelling_core::validation::schema::validate_odps_internal,
    },
    Corpus {
        crate_dir: "crates/conform-lexicon",
        output: "crates/conform-lexicon/tests/oracle/odcl-verdicts.json",
        function: "data_modelling_core::validation::schema::validate_odcl_internal",
        // Unlike its ODCS sibling, this function does not sniff: it compiles
        // the ODCL schema and validates against it, and that is all it does.
        // Which is why its verdict is a clean oracle for `conform-lexicon` —
        // there is no dispatch to see through.
        sdk_schema: "schemas/odcl-json-schema-1.2.1.json",
        vendored_schema: "schemas/odcl-json-schema-1.2.1.json",
        validate: data_modelling_core::validation::schema::validate_odcl_internal,
    },
];

fn main() {
    let repo = repo_root();
    let sdk = sdk_root();
    let sdk_commit = git_commit(&sdk);

    for corpus in CORPORA {
        let fixtures_dir = repo.join(corpus.crate_dir).join("tests/fixtures");
        let mut fixtures: Vec<PathBuf> = fs::read_dir(&fixtures_dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", fixtures_dir.display()))
            .map(|entry| entry.expect("cannot read directory entry").path())
            .filter(|path| path.is_file())
            .collect();
        fixtures.sort();

        let recorded: Vec<Value> = fixtures
            .iter()
            .map(|path| {
                let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                let text = String::from_utf8(bytes.clone())
                    .unwrap_or_else(|e| panic!("{} is not UTF-8: {e}", path.display()));
                let name = path
                    .file_name()
                    .expect("a file has a name")
                    .to_string_lossy()
                    .into_owned();

                match (corpus.validate)(&text) {
                    Ok(()) => json!({
                        "fixture": name,
                        "sha256": sha256_hex(&bytes),
                        "verdict": "ok",
                        "detail": Value::Null,
                    }),
                    Err(message) => json!({
                        "fixture": name,
                        "sha256": sha256_hex(&bytes),
                        "verdict": "err",
                        "detail": message,
                    }),
                }
            })
            .collect();

        let total = recorded.len();
        let failures = recorded.iter().filter(|f| f["verdict"] == "err").count();

        let record = json!({
            "schema_version": 1,
            "recorded_by": "tools/oracle",
            "how_to_regenerate": "cargo run --manifest-path tools/oracle/Cargo.toml",
            "oracle": {
                "crate": "data-modelling-core",
                "crate_version": sdk_version(&sdk),
                "function": corpus.function,
                "features": ["schema-validation"],
                "source_repository": "data-modelling-sdk",
                "source_commit": sdk_commit,
                "schema_it_reads": corpus.sdk_schema,
                "schema_it_reads_sha256": sha256_hex(
                    &fs::read(sdk.join(corpus.sdk_schema))
                        .unwrap_or_else(|e| panic!("cannot read the SDK's {}: {e}", corpus.sdk_schema)),
                ),
            },
            "vendored_schema": {
                "path": corpus.vendored_schema,
                "sha256": sha256_hex(
                    &fs::read(repo.join(corpus.vendored_schema))
                        .unwrap_or_else(|e| panic!("cannot read {}: {e}", corpus.vendored_schema)),
                ),
            },
            "fixtures": recorded,
        });

        let output = repo.join(corpus.output);
        fs::create_dir_all(output.parent().expect("the output path has a directory"))
            .expect("cannot create the output directory");
        let mut text = serde_json::to_string_pretty(&record).expect("the record serializes");
        text.push('\n');
        fs::write(&output, text).unwrap_or_else(|e| panic!("cannot write {}: {e}", output.display()));

        println!(
            "{}: {total} fixtures recorded ({} ok, {failures} err) -> {}",
            corpus.function,
            total - failures,
            corpus.output
        );
    }
}

/// The version the SDK's core crate declares, read from its manifest rather
/// than transcribed — a transcribed version is a version that goes stale.
fn sdk_version(sdk: &Path) -> Value {
    let manifest = sdk.join("crates/core/Cargo.toml");
    fs::read_to_string(&manifest)
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("version = "))
                .map(|value| value.trim().trim_matches('"').to_owned())
        })
        .map_or(Value::Null, Value::String)
}

/// The repository this tool lives in — two directories above its manifest.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("tools/oracle sits two levels below the repository root")
        .to_path_buf()
}

/// The SDK checkout the oracle is compiled from.
///
/// A sibling checkout, which is an assumption about one machine's layout and
/// is stated as such in `README.md`. Nothing in the gated workspace depends on
/// it; only regenerating the recording does.
fn sdk_root() -> PathBuf {
    repo_root()
        .parent()
        .expect("the repository has a parent directory")
        .join("data-modelling-sdk")
}

/// The SDK's current commit, so the recording says which implementation it
/// recorded rather than "the one that was there".
fn git_commit(sdk: &Path) -> Value {
    Command::new("git")
        .args(["-C", &sdk.display().to_string(), "rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map_or(Value::Null, |sha| Value::String(sha.trim().to_owned()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}
