# conform-core

The shared vocabulary for a family of conformance validators: `Diagnostic`,
`Severity`, `Location`, `ConformanceReport`, the `Validator` trait, and a
three-way `Resolution` for cross-reference checks.

This crate knows nothing about any specific document standard, and that is the
whole point of it. Every rule, every schema, every parser lives in an adapter
crate layered on top; `conform-core` owns only the machinery those adapters
would otherwise each reinvent — how a diagnostic is shaped, how reporting
differs from gating, and how "the target is absent" is kept distinct from "the
target was never looked at".

Two invariants are enforced by tests rather than by review:

- **No standard-specific leakage.** A test reads this crate's own sources and
  fails on any mention of a specific document standard, so the boundary holds
  across future adapters.
- **Three-way resolution.** A test asserts that "does not exist" and "not
  inspected" cannot collapse into each other. Collapsing them is the exact
  defect class this crate exists to prevent: a gate that cannot tell the two
  apart cries wolf, and a gate that cries wolf stops being trusted.

## Dependencies

Zero non-`std` runtime dependencies by default. `serde` is available behind an
off-by-default `serde` feature and affects serialization only — never which
diagnostics are produced, never whether a report gates.

## Licence

Dual licensed under `MIT OR Apache-2.0`.
