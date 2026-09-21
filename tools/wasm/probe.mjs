// Instantiate the WebAssembly binding and call it, in Node.
//
//   node tools/wasm/probe.mjs <dir with conform_ffi.js built for --target nodejs>
//
// Not a `#[test]`. A cargo test runs on the host, where `catch_unwind` works,
// `JsError` cannot be constructed and nothing traps — which means the three
// things this file exists to measure are precisely the three a cargo test
// cannot reach. `crates/conform-ffi/src/wasm.rs` says so in its test module
// and points here.
//
// # The part that matters: a probe that did not run must not look clean
//
// Every check below increments a counter. If the counter is zero at the end,
// or any check failed, the process exits non-zero. A green tick from this file
// means the module was instantiated and called, not that the file was reached.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

let ran = 0;
let failed = 0;

function check(what, condition, detail) {
  ran += 1;
  if (condition) {
    console.log(`  ok    ${what}${detail === undefined ? "" : ` — ${detail}`}`);
  } else {
    failed += 1;
    console.log(`  FAIL  ${what}${detail === undefined ? "" : ` — ${detail}`}`);
  }
}

// An observation rather than an assertion. Some of what this probe finds out
// is behaviour we describe but deliberately do not depend on — see the
// aftermath of the trap below — and recording it as a check would be asserting
// that a hazard stays exactly as hazardous as it is today.
function observe(what, value) {
  console.log(`  note  ${what} — ${value}`);
}

const dir = process.argv[2];
if (!dir) {
  console.error("usage: node tools/wasm/probe.mjs <out-dir from tools/wasm/build.sh --target nodejs>");
  process.exit(2);
}

const root = resolve(new URL("../..", import.meta.url).pathname);
const glue = resolve(dir, "conform_ffi.js");
const module = await import(pathToFileURL(glue).href);

console.log(`probe: ${glue}`);

// ---------------------------------------------------------------------------
// 1. The ABI crosses at all.
// ---------------------------------------------------------------------------
const manifest = readFileSync(resolve(root, "crates/conform-ffi/Cargo.toml"), "utf8");
const expectedVersion = /^version = "([^"]+)"$/m.exec(manifest)?.[1];

check(
  "conformVersion() is this crate's version",
  module.conformVersion() === expectedVersion,
  `${module.conformVersion()} vs ${expectedVersion}`,
);
check(
  "envelopeSchemaVersion() is a number",
  Number.isInteger(module.envelopeSchemaVersion()),
  String(module.envelopeSchemaVersion()),
);

// ---------------------------------------------------------------------------
// 2. The embedded schemas, and the digest gate in front of them.
// ---------------------------------------------------------------------------
const ids = module.reachableSpecIds();
check("reachableSpecIds() are the embedded ones", ids.join(",") === "odcs,odps", ids.join(","));

for (const id of ids) {
  const validator = new module.ConformValidator(id);
  check(`new ConformValidator("${id}") builds without a filesystem`, validator.specId === id);
  check(
    `the ${id} provenance sentence says the bytes were embedded and re-hashed`,
    validator.provenance.includes("embedded in this build") &&
      validator.provenance.includes("re-hashed"),
    validator.provenance.slice(0, 72) + "…",
  );
  validator.free();
}

// ---------------------------------------------------------------------------
// 3. A real document, and a real verdict.
// ---------------------------------------------------------------------------
const odcs = new module.ConformValidator("odcs");
const faulty = readFileSync(
  resolve(root, "crates/conform-odcs/tests/fixtures/faulty-many-faults.yaml"),
  "utf8",
);
const report = JSON.parse(odcs.validate("faulty-many-faults.yaml", faulty));

check("the report is this envelope", report.schema_version === module.envelopeSchemaVersion());
check("the report names the document it was given", report.document.id === "faulty-many-faults.yaml");
check("a faulty contract is reported as an error", report.document.worst_severity === "error");
check(
  "the schema's own findings came across",
  report.diagnostics.some((d) => d.code.startsWith("ODCS1")),
  report.diagnostics.map((d) => d.code).join(","),
);
check(
  "every report carries the schema it was validated against",
  report.diagnostics.some((d) => d.code === "ODCS904"),
);

// A clean document is a success with no errors — the same rule the C ABI and
// the CLI follow, checked here so the browser cannot be the one place where
// "no findings" and "could not validate" look alike.
const clean = JSON.parse(
  odcs.validate(
    "conformant-full.yaml",
    readFileSync(resolve(root, "crates/conform-odcs/tests/fixtures/conformant-full.yaml"), "utf8"),
  ),
);
check("a clean contract raises no errors", clean.document.error === 0, JSON.stringify(clean.document));
odcs.free();

// ---------------------------------------------------------------------------
// 4. A refusal is a thrown error, not a half-built validator.
// ---------------------------------------------------------------------------
let threw = null;
try {
  new module.ConformValidator("okf");
} catch (error) {
  threw = error;
}
check("an unembedded spec id throws", threw !== null, threw && String(threw.message).slice(0, 80));
check(
  "and the refusal names what this build does carry",
  threw !== null && String(threw.message).includes("odcs"),
);

// ---------------------------------------------------------------------------
// 5. The panic contract. This is the reason the file exists.
// ---------------------------------------------------------------------------
const contract = module.panicContract();
check("panicContract() calls a panic fatal", contract.includes("fatal"));
check("panicContract() does not promise a caught panic", !contract.includes("catch_unwind` to catch."));
check(
  "the wasm surface exports no self-test that claims to catch",
  module.conformSelfTestPanic === undefined && module.selfTestPanic === undefined,
  "conformSelfTestPanic is absent by design",
);

let trap = null;
try {
  module.demonstratePanicIsFatal();
} catch (error) {
  trap = error;
}
check("demonstratePanicIsFatal() does not return", trap !== null);
check(
  "it traps rather than throwing a Rust-shaped error",
  trap instanceof WebAssembly.RuntimeError,
  trap && `${trap.constructor.name}: ${trap.message}`,
);

// The aftermath, recorded and not asserted. WebAssembly does not stop an
// instance that traps, so these calls are expected to *work* — and that is the
// hazard the contract describes, because the panicking call's state was
// abandoned and nothing here can see it. Asserting either outcome would be
// making a promise about a hazard.
let stillAnswers;
try {
  stillAnswers = `conformVersion() returned ${module.conformVersion()}`;
} catch (error) {
  stillAnswers = `conformVersion() threw ${error.constructor.name}`;
}
observe("after the trap, the same instance", stillAnswers);

// ---------------------------------------------------------------------------
console.log(`probe: ${ran} checks, ${failed} failed`);
if (ran === 0) {
  console.error("probe: nothing ran, which is not the same as nothing failed");
  process.exit(1);
}
process.exit(failed === 0 ? 0 : 1);
