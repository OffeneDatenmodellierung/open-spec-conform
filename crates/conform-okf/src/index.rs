//! Reading what an `index.md` lists.
//!
//! Shared by the two rules that read a listing — "this index names something
//! gone" (`OKF407`, conformance) and "no index names this concept" (`OKFL09`,
//! hygiene). They are opposite directions of one question, and two copies of
//! the walk would be two chances to resolve a link differently and report a
//! contradiction.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use okf_core::Bundle;

/// Every concept id any `index.md` links to.
pub(crate) fn indexed_concepts(bundle: &Bundle) -> BTreeSet<String> {
    let mut listed = BTreeSet::new();
    for index in bundle.index_files() {
        for (_, resolved) in index_listings(bundle, index) {
            if let Ok(id) = okf_core::ConceptId::from_path(bundle.root(), &resolved) {
                listed.insert(id.to_string());
            }
        }
    }
    listed
}

/// Every concept document one `index.md` links to, as `(as written,
/// resolved)`.
///
/// An index that cannot be read yields nothing rather than an error. That is
/// deliberate and it is the conservative direction: the rules built on this
/// both report *absence* of a target, so a failed read here can only ever
/// under-report. Turning an unreadable index into a finding of its own is a
/// different rule, and adding it during a migration would make the migration
/// uncheckable against what it replaced.
pub(crate) fn index_listings(bundle: &Bundle, index: &Path) -> Vec<(String, PathBuf)> {
    let Ok(text) = std::fs::read_to_string(index) else {
        return Vec::new();
    };
    let parent = index.parent().unwrap_or_else(|| bundle.root());
    okf_core::links::extract_links(&text)
        .into_iter()
        .filter_map(|link| {
            let target = link.target_without_anchor().to_owned();
            if target.contains("://")
                || !Path::new(&target)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("md"))
            {
                return None;
            }
            let resolved = target
                .strip_prefix('/')
                .map_or_else(|| parent.join(&target), |rooted| bundle.root().join(rooted));
            Some((target, resolved))
        })
        .collect()
}
