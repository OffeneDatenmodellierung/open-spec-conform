//! The three-way outcome of a cross-reference check.

use core::fmt;

/// Why a reference target was never looked at.
///
/// Marked `#[non_exhaustive]`: new reasons are additive, and a consumer that
/// matches on these must say what it does with one it has not heard of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[non_exhaustive]
pub enum NotInspectedReason {
    /// The target lies outside the set of documents that was loaded, so this
    /// run has no way to know whether it exists.
    OutsideDocumentSet,
    /// The target is inside the set but was not loaded on this run.
    NotLoaded,
    /// The target was deliberately skipped — excluded by configuration, or
    /// filtered out of this run.
    Excluded,
    /// Inspection was attempted and failed: unreadable, unparseable, or
    /// otherwise unusable as evidence either way.
    InspectionFailed,
    /// The validator does not know how to follow a reference of this kind.
    Unsupported,
}

impl NotInspectedReason {
    /// A stable, machine-readable name for this reason.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OutsideDocumentSet => "outside-document-set",
            Self::NotLoaded => "not-loaded",
            Self::Excluded => "excluded",
            Self::InspectionFailed => "inspection-failed",
            Self::Unsupported => "unsupported",
        }
    }
}

impl fmt::Display for NotInspectedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What happened when a cross-reference was followed.
///
/// There are **three** outcomes here, not two, and that is the entire reason
/// this type exists:
///
/// - [`Resolution::Resolved`] — the target was looked for and found.
/// - [`Resolution::DoesNotExist`] — the target was looked for and is genuinely
///   absent from the document set. This is a finding.
/// - [`Resolution::NotInspected`] — nobody looked. This is **not** a finding;
///   it is an absence of evidence.
///
/// The last two are what a two-valued `Option`-shaped answer silently fuses,
/// and fusing them is the defect this crate is here to prevent: a gate that
/// reports "broken reference" when the truth is "I never opened that document"
/// raises a false alarm, and a gate that raises false alarms gets ignored,
/// after which it protects nothing at all.
///
/// Note what is deliberately *missing* from this type: there is no conversion
/// to `Option`, no `is_missing`, and no helper that answers "did it resolve?"
/// with a `bool`. Every one of those would let the two non-resolving cases be
/// collapsed by accident at a call site. Ask the specific question you mean:
/// [`Resolution::does_not_exist`] or [`Resolution::is_not_inspected`].
///
/// ```
/// use conform_core::{NotInspectedReason, Resolution};
///
/// let found: Resolution<u32> = Resolution::Resolved(7);
/// let absent: Resolution<u32> = Resolution::DoesNotExist;
/// let unknown: Resolution<u32> =
///     Resolution::not_inspected(NotInspectedReason::OutsideDocumentSet);
///
/// assert_eq!(found.resolved(), Some(&7));
/// assert!(absent.does_not_exist());
/// assert!(unknown.is_not_inspected());
///
/// // The two non-resolving outcomes are different facts, and stay different.
/// assert_ne!(absent, unknown);
/// assert!(!absent.is_not_inspected());
/// assert!(!unknown.does_not_exist());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum Resolution<T> {
    /// The target was found. Carries whatever the validator wants to hand back
    /// about it — an identifier, a borrowed node, `()` if the fact of
    /// resolution is all that matters.
    Resolved(T),
    /// The target was looked for across the inspected document set and is not
    /// there.
    DoesNotExist,
    /// The target's existence is unknown, because it was never inspected.
    NotInspected {
        /// Why it was not inspected.
        reason: NotInspectedReason,
    },
}

impl<T> Resolution<T> {
    /// The "nobody looked" outcome, with its reason.
    #[must_use]
    pub const fn not_inspected(reason: NotInspectedReason) -> Self {
        Self::NotInspected { reason }
    }

    /// Whether the target was found.
    #[must_use]
    pub const fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved(_))
    }

    /// Whether the target was looked for and is genuinely absent.
    ///
    /// False for [`Resolution::NotInspected`], which is the point.
    #[must_use]
    pub const fn does_not_exist(&self) -> bool {
        matches!(self, Self::DoesNotExist)
    }

    /// Whether nobody looked.
    ///
    /// False for [`Resolution::DoesNotExist`], which is the point.
    #[must_use]
    pub const fn is_not_inspected(&self) -> bool {
        matches!(self, Self::NotInspected { .. })
    }

    /// What was found, if anything was.
    ///
    /// This narrows the *payload*, not the outcome: `None` here means only
    /// "there is no target to hand you", and says nothing about which of the
    /// two non-resolving outcomes occurred. Never use it to decide whether to
    /// raise a diagnostic.
    #[must_use]
    pub const fn resolved(&self) -> Option<&T> {
        match self {
            Self::Resolved(target) => Some(target),
            _ => None,
        }
    }

    /// Why the target was not inspected, if that is what happened.
    #[must_use]
    pub const fn not_inspected_reason(&self) -> Option<NotInspectedReason> {
        match self {
            Self::NotInspected { reason } => Some(*reason),
            _ => None,
        }
    }

    /// Transform the resolved target, leaving both non-resolving outcomes
    /// exactly as they are.
    #[must_use]
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Resolution<U> {
        match self {
            Self::Resolved(target) => Resolution::Resolved(f(target)),
            Self::DoesNotExist => Resolution::DoesNotExist,
            Self::NotInspected { reason } => Resolution::NotInspected { reason },
        }
    }

    /// Borrow the resolved target, keeping all three outcomes intact.
    ///
    /// Named `by_ref` rather than `as_ref` so it cannot be mistaken for an
    /// [`AsRef`] conversion.
    #[must_use]
    pub const fn by_ref(&self) -> Resolution<&T> {
        match self {
            Self::Resolved(target) => Resolution::Resolved(target),
            Self::DoesNotExist => Resolution::DoesNotExist,
            Self::NotInspected { reason } => Resolution::NotInspected { reason: *reason },
        }
    }
}
