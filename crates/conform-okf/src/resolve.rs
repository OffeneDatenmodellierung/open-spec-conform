//! Following a path a bundle names, and saying which of three things happened.

use std::path::{Component, Path, PathBuf};

use conform_core::{NotInspectedReason, Resolution};
use okf_core::Bundle;

/// Follow a `resource:`-style path against a bundle.
///
/// The three outcomes are the reason this returns
/// [`Resolution`](conform_core::Resolution) rather than an `Option`:
///
/// - [`Resolution::Resolved`] — the bundle contains the file, and here is
///   where.
/// - [`Resolution::DoesNotExist`] — the path names something inside the
///   bundle, the bundle was asked, and it is not there. A finding.
/// - [`Resolution::NotInspected`] — the path names something the bundle does
///   not own: a URL, a `mailto:`, or a path that climbs out of the root.
///   **Not** a finding. Whether `https://…` resolves is a network question and
///   this crate is offline by construction, so the honest answer is that
///   nobody looked.
///
/// The implementation this crate is migrated from made the same three-way
/// decision and expressed the third case as a bare `continue`, which is
/// correct behaviour with the reasoning left in a comment. Nothing about the
/// output changes here; what changes is that "we did not look" is now a value
/// a caller can be handed rather than a branch a caller can forget.
///
/// ```
/// use conform_core::{NotInspectedReason, Resolution};
/// use conform_okf::{Bundle, resolve_resource};
///
/// let bundle = Bundle::load(
///     concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/okf-upstream/acme_retail"),
/// )?;
///
/// assert!(resolve_resource(&bundle, "tables/orders.md").is_resolved());
/// assert!(resolve_resource(&bundle, "tables/nothing-here.md").does_not_exist());
///
/// // A URL is not absent. Nobody looked, and the reason is on the record.
/// let remote = resolve_resource(&bundle, "https://example.org/orders");
/// assert!(remote.is_not_inspected());
/// assert_eq!(
///     remote.not_inspected_reason(),
///     Some(NotInspectedReason::OutsideDocumentSet),
/// );
/// # Ok::<(), okf_core::BundleError>(())
/// ```
#[must_use]
pub fn resolve_resource(bundle: &Bundle, raw: &str) -> Resolution<PathBuf> {
    let Some(relative) = bundle_relative(raw) else {
        return Resolution::not_inspected(NotInspectedReason::OutsideDocumentSet);
    };
    let path = bundle.root().join(&relative);
    if path.exists() {
        Resolution::Resolved(path)
    } else {
        Resolution::DoesNotExist
    }
}

/// The bundle-relative path a `resource:` names, or `None` when it names
/// something outside the bundle — a URL, or a path that climbs out of it.
///
/// Nothing that could climb out of the bundle or re-root the join is accepted,
/// on **either** platform's rules. A bundle is a portable artefact — one
/// written on Windows is read on Unix — so the separator cannot be left to
/// whichever machine happens to be reading. `..\..` is a single ordinary
/// filename to Unix and a climb to Windows, and `C:\…` re-roots the join
/// outright; the caller does `bundle.root().join(relative)`, so either would
/// have this crate stat a file the bundle does not own.
#[must_use]
pub fn bundle_relative(raw: &str) -> Option<String> {
    if raw.contains("://") || raw.starts_with("mailto:") {
        return None;
    }
    let trimmed = raw.trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    if trimmed
        .split(['/', '\\'])
        .any(|segment| segment == ".." || segment == "." || segment.is_empty())
    {
        return None;
    }
    // A drive or UNC prefix. A URL was already excluded above, and no portable
    // filename carries a colon, so this costs nothing that was readable anyway.
    if trimmed.contains(':') {
        return None;
    }
    // The platform's own reading, as a backstop: whatever the two rules above
    // missed, every component must still be an ordinary name.
    if Path::new(trimmed)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(trimmed.to_owned())
}
