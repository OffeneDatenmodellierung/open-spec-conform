//! Making third-party text safe to put on a terminal, exactly once.
//!
//! # The obligation, and where it comes from
//!
//! Every adapter in this family quotes the document it found a fault in — a
//! title, an actor id, a path, a frontmatter scalar — because a finding that
//! does not quote what it found is not actionable. None of them sanitises
//! those values, and `conform-okf` says so in as many words:
//!
//! > A bundle can therefore carry a bidirectional override or a zero-width run
//! > into a message, and a renderer that writes it straight to a terminal will
//! > show a line the bundle rewrote. That is the renderer's boundary, not this
//! > crate's […] escaping is not idempotent […] so it has to happen exactly
//! > once, at the point of display.
//!
//! **This crate is the renderer.** The obligation lands here and nowhere else.
//!
//! # What a hostile document can do to a terminal
//!
//! Three families, all of which have been used in the wild:
//!
//! - **ANSI control sequences** (`ESC [ … m`, `ESC ] … BEL`). At minimum these
//!   recolour the operator's screen; at worst `ESC ] 0 ; … BEL` rewrites the
//!   window title and some terminals still answer device-control queries by
//!   injecting text back onto the shell's input.
//! - **Bidirectional overrides** (U+202E and relatives, the "Trojan Source"
//!   family). These reorder the glyphs of a line without changing its bytes,
//!   so a diagnostic can be made to *read* as the opposite of what it says.
//! - **Invisible runs** — zero-width spaces and joiners, the BOM, line and
//!   paragraph separators. Text that is present, consequential, and impossible
//!   to see.
//!
//! # The rule this module implements
//!
//! Escape **once**, at the boundary, in **terminal** renderings only:
//!
//! | Sink | Escaped here | Why |
//! |---|---|---|
//! | human-readable stdout | yes | it is a terminal |
//! | the TUI | yes | it is a terminal |
//! | `--json` | **no** | JSON string encoding is already the correct escaping for that sink, and applying both corrupts the data — a consumer would read `<U+202E>` where the document held one character |
//!
//! Note the asymmetry is the point rather than an oversight: the JSON consumer
//! is not a terminal, gets the bytes the document really held, and owns its own
//! boundary. A test in `tests/hostile_text_is_neutralised.rs` holds both halves
//! of that table at once.
//!
//! # No ANSI is emitted by the human renderer at all
//!
//! Deliberately. The human-readable output carries no colour — severity is
//! carried by a word and a glyph, which is also what plan §4.2 asks for on
//! accessibility grounds, and what makes `NO_COLOR` trivially honoured. The
//! security consequence is the one worth stating: the human renderer never
//! writes an escape byte of its own, so **any** `ESC` reaching that stream
//! would have had to come from a document — and this module is what guarantees
//! none does.

use std::borrow::Cow;
use std::fmt::Write as _;

/// Rewrite text so that a terminal displays it rather than obeys it.
///
/// Every character this module considers dangerous is replaced by a visible,
/// unambiguous `<U+XXXX>` spelling of its code point. Everything else — every
/// letter, mark, emoji, CJK ideograph and ordinary space — is passed through
/// untouched, because mangling legitimate text is its own kind of dishonesty:
/// an operator who cannot read the value the document actually held cannot act
/// on the finding.
///
/// Borrows when nothing needed escaping, which is the overwhelmingly common
/// case and keeps this free on the ordinary path.
///
/// ```
/// use conform_cli::escape::for_terminal;
///
/// // Ordinary text is returned untouched, and not reallocated.
/// assert!(matches!(for_terminal("servers[0].host is required"), std::borrow::Cow::Borrowed(_)));
///
/// // A right-to-left override cannot reorder the line any more.
/// assert_eq!(for_terminal("id: \u{202e}drowssap"), "id: <U+202E>drowssap");
///
/// // Neither can a colour sequence recolour the operator's screen.
/// assert_eq!(for_terminal("\u{1b}[31mDANGER\u{1b}[0m"), "<U+001B>[31mDANGER<U+001B>[0m");
/// ```
#[must_use]
pub fn for_terminal(text: &str) -> Cow<'_, str> {
    if !text.chars().any(is_dangerous) {
        return Cow::Borrowed(text);
    }

    let mut escaped = String::with_capacity(text.len() + 8);
    for character in text.chars() {
        if is_dangerous(character) {
            // `{:04X}` is the conventional spelling of a code point, and the
            // angle brackets are there so a reader can tell an escape from a
            // document that happens to contain the letters `U+202E`.
            // Writing into a `String` cannot fail, so the result is deliberately
            // discarded rather than unwrapped.
            let _ = write!(escaped, "<U+{:04X}>", character as u32);
        } else {
            escaped.push(character);
        }
    }
    Cow::Owned(escaped)
}

/// Whether a character must not reach a terminal verbatim.
///
/// Three groups, and the third is the one people forget.
#[must_use]
pub fn is_dangerous(character: char) -> bool {
    // 1. C0 and C1 controls, which `char::is_control` covers exactly. This
    //    takes `\n`, `\r` and `\t` with it, and that is wanted: a diagnostic is
    //    rendered as a line, and a message carrying a newline would otherwise
    //    forge a second diagnostic in the output.
    if character.is_control() {
        return true;
    }

    matches!(
        character,
        // 2. Bidirectional formatting — the Trojan Source family. Marks,
        //    embeddings, overrides, isolates, and the pops that end them.
        '\u{200E}' | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            // 3. Invisible but consequential: zero-width space, the two
            //    joiners, the word joiner, the byte-order mark in its
            //    zero-width-no-break-space role, and the soft hyphen.
            | '\u{200B}'..='\u{200D}'
            | '\u{2060}'
            | '\u{FEFF}'
            | '\u{00AD}'
            // Line and paragraph separators: not `is_control`, but a terminal
            // and most of what reads a terminal treat them as line breaks.
            | '\u{2028}' | '\u{2029}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_text_is_untouched_and_unallocated() {
        for sample in [
            "",
            "apiVersion: v3.1.0",
            "Grüße aus München",
            "注文 — orders",
            "emoji are text: 🛰️",
            "backslashes \\ and quotes \" survive",
        ] {
            assert!(
                matches!(for_terminal(sample), Cow::Borrowed(_)),
                "{sample:?} should not have been rewritten"
            );
        }
    }

    #[test]
    fn every_dangerous_family_is_neutralised() {
        for (raw, expected) in [
            ("\u{1b}]0;title\u{7}", "<U+001B>]0;title<U+0007>"),
            ("line\nbreak", "line<U+000A>break"),
            ("tab\there", "tab<U+0009>here"),
            ("\u{202e}override", "<U+202E>override"),
            ("\u{2066}isolate\u{2069}", "<U+2066>isolate<U+2069>"),
            ("zero\u{200b}width", "zero<U+200B>width"),
            ("\u{feff}bom", "<U+FEFF>bom"),
            ("soft\u{ad}hyphen", "soft<U+00AD>hyphen"),
            ("para\u{2029}separator", "para<U+2029>separator"),
            ("\u{7f}delete", "<U+007F>delete"),
            ("\u{9b}c1-csi", "<U+009B>c1-csi"),
        ] {
            assert_eq!(for_terminal(raw), expected, "escaping {raw:?}");
        }
    }

    #[test]
    fn nothing_dangerous_survives_a_pass() {
        // The property, rather than the table: whatever goes in, no character
        // this module calls dangerous comes out.
        let hostile: String = (0u32..0x3000)
            .chain([0xFEFF])
            .filter_map(char::from_u32)
            .collect();
        assert!(!for_terminal(&hostile).chars().any(is_dangerous));
    }

    #[test]
    fn the_escape_is_visible_rather_than_silent() {
        // Deleting a dangerous character would be the other obvious fix, and it
        // is the wrong one: the operator would be told a different string was
        // in the document than the one that was. Length grows; it never shrinks.
        for raw in ["\u{202e}", "\u{1b}", "\u{200b}"] {
            assert!(for_terminal(raw).len() > raw.len());
            assert!(for_terminal(raw).contains("U+"));
        }
    }
}
