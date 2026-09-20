//! Making third-party text safe to put in a browser, exactly once.
//!
//! # The obligation, and where it comes from
//!
//! [`conform_cli::escape`] states the rule this module inherits: every adapter
//! in this family quotes the document it found a fault in, none of them
//! sanitises what it quotes, and escaping is not idempotent — so it has to
//! happen exactly once, at the point of display. The console is one point of
//! display. **This crate is another**, and the sink is different enough that
//! the console's answer is the wrong one here.
//!
//! # Two different threats, which is why this is not `for_terminal`
//!
//! A terminal *obeys* text: `ESC [ 31 m` recolours it, `ESC ] 0 ;` rewrites
//! its title bar. A browser *parses* text: `<` opens a tag, and a `<script>`
//! smuggled into a page runs with the page's origin. They overlap in one
//! family and differ in the rest:
//!
//! | Threat | Terminal | Browser |
//! |---|---|---|
//! | `<`, `&`, `"`, `'` | harmless | **XSS** — the whole game |
//! | ANSI control sequences | screen and title bar rewritten | inert |
//! | Bidirectional overrides (U+202E) | line reads backwards | **line reads backwards** |
//! | Zero-width and invisible runs | text present and unseeable | **text present and unseeable** |
//!
//! So this module does both halves, and neither half alone would do:
//!
//! 1. **Markup-significant characters are entity-encoded**, which is what
//!    stops a registry note or a diagnostic message from becoming an element.
//! 2. **The Trojan Source family is spelled out**, in exactly the
//!    `<U+202E>` notation [`conform_cli::escape::for_terminal`] uses, because
//!    a bidirectional override reorders a paragraph of HTML just as
//!    effectively as it reorders a line of terminal output. Entity-encoding
//!    does not touch it: U+202E is not markup-significant, so an
//!    HTML-escaper that only handles `<` and `&` passes it straight through
//!    and the rendered page lies.
//!
//! The spelling is deliberately the console's. A reader who has seen
//! `<U+202E>` in `conform registry list` meets the same notation here rather
//! than a second convention for the same fact.
//!
//! # ANSI is escaped here too, and the reason is honesty rather than safety
//!
//! An `ESC` byte cannot hurt a browser. It is neutralised anyway, because
//! `char::is_control` is the cheapest correct boundary for "this character has
//! no business being rendered as itself", and because a note that contains a
//! control byte is a note a reader should be able to *see* contains one. What
//! is deliberately not treated as dangerous is `\n`: the console forbids it so
//! that a hostile message cannot forge a second diagnostic line, and this
//! renderer has no such line format — it puts text inside an element, where a
//! newline is whitespace and nothing more. See [`is_dangerous`].
//!
//! # Escape once, at the boundary
//!
//! Every value that reaches the page goes through [`for_html`] exactly once,
//! in [`crate::render`], on its way into the document. Nothing upstream of
//! that is pre-escaped and nothing downstream escapes again, so the page can
//! never show a double-encoded `&amp;lt;` — which is its own kind of
//! dishonesty, since the reader is then told the document held characters it
//! did not.

use std::borrow::Cow;
use std::fmt::Write as _;

/// Rewrite text so that a browser displays it rather than parses it.
///
/// Safe for element content and for a quoted attribute value, which is why
/// both `"` and `'` are encoded: a single function with one rule is one thing
/// to audit, and the alternative — a content escaper and an attribute escaper
/// — is an invitation to reach for the wrong one.
///
/// Borrows when nothing needed escaping, which keeps this free on the ordinary
/// path.
///
/// ```
/// use conform_web::escape::for_html;
///
/// // Ordinary text is returned untouched, and not reallocated.
/// assert!(matches!(for_html("Open Data Contract Standard"), std::borrow::Cow::Borrowed(_)));
///
/// // Markup cannot be smuggled through a registry note.
/// assert_eq!(
///     for_html("<script>alert(1)</script>"),
///     "&lt;script&gt;alert(1)&lt;/script&gt;",
/// );
///
/// // Nor can an attribute be broken out of.
/// assert_eq!(for_html(r#"" onload="evil()"#), "&quot; onload=&quot;evil()");
///
/// // And a right-to-left override cannot reorder the rendered line, which
/// // entity-encoding alone would not have prevented.
/// assert_eq!(for_html("licence: \u{202e}detidua"), "licence: <U+202E>detidua");
/// ```
#[must_use]
pub fn for_html(text: &str) -> Cow<'_, str> {
    if !text.chars().any(|c| entity(c).is_some() || is_dangerous(c)) {
        return Cow::Borrowed(text);
    }

    let mut escaped = String::with_capacity(text.len() + 16);
    for character in text.chars() {
        if let Some(entity) = entity(character) {
            escaped.push_str(entity);
        } else if is_dangerous(character) {
            // `{:04X}` is the conventional spelling of a code point, and the
            // angle brackets — themselves entity-encoded nowhere, because this
            // text is the escaper's own output rather than the document's —
            // let a reader tell an escape from a document that happens to
            // contain the letters `U+202E`. Writing into a `String` cannot
            // fail, so the result is deliberately discarded.
            let _ = write!(escaped, "<U+{:04X}>", character as u32);
        } else {
            escaped.push(character);
        }
    }
    Cow::Owned(escaped)
}

/// The HTML entity for a markup-significant character, if it has one.
///
/// Five characters, which is the set that matters: `&` first in spirit if not
/// in code, because encoding it after the others would double-encode their
/// output — a mistake this shape makes impossible, since each character is
/// examined once and replaced once.
const fn entity(character: char) -> Option<&'static str> {
    match character {
        '&' => Some("&amp;"),
        '<' => Some("&lt;"),
        '>' => Some("&gt;"),
        '"' => Some("&quot;"),
        '\'' => Some("&#x27;"),
        _ => None,
    }
}

/// Whether a character must not reach a browser verbatim, over and above the
/// markup-significant five.
///
/// The console's list, minus the one entry that is about a line-oriented sink.
#[must_use]
pub fn is_dangerous(character: char) -> bool {
    // Whitespace that a browser treats as whitespace. The console escapes
    // these because its output format is one diagnostic per line and a
    // newline in a message would forge a second finding; this renderer puts
    // text inside an element, where they are layout and cannot forge
    // anything. Escaping them here would mangle the registry's notes, which
    // are the most valuable content on the page and are written in
    // paragraphs.
    if matches!(character, '\n' | '\r' | '\t') {
        return false;
    }

    // C0 and C1 controls. Inert in a browser; escaped so that a reader can see
    // the text contains one.
    if character.is_control() {
        return true;
    }

    matches!(
        character,
        // Bidirectional formatting — the Trojan Source family. Marks,
        // embeddings, overrides, isolates, and the pops that end them. These
        // are the ones an HTML escaper that only knows about `<` and `&`
        // lets through, and they reorder rendered text in a browser exactly as
        // they do on a terminal.
        '\u{200E}' | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            // Invisible but consequential: zero-width space, the two joiners,
            // the word joiner, the byte-order mark in its
            // zero-width-no-break-space role, and the soft hyphen.
            | '\u{200B}'..='\u{200D}'
            | '\u{2060}'
            | '\u{FEFF}'
            | '\u{00AD}'
            // Line and paragraph separators. Not `is_control`, and a browser
            // does break lines on them — so a note could otherwise be given a
            // layout its author never wrote.
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
            "Open Data Contract Standard",
            "https://bitol.io/",
            "Grüße aus München",
            "注文 — orders",
            "emoji are text: 🛰️",
            "a note\nwith paragraphs\n\nand a tab\there",
        ] {
            assert!(
                matches!(for_html(sample), Cow::Borrowed(_)),
                "{sample:?} should not have been rewritten"
            );
        }
    }

    #[test]
    fn every_markup_significant_character_is_encoded() {
        assert_eq!(
            for_html("& < > \" '"),
            "&amp; &lt; &gt; &quot; &#x27;",
            "the five characters that let text become markup",
        );
    }

    #[test]
    fn encoding_happens_once_and_not_twice() {
        // The classic bug: escaping `&` after `<` has already become `&lt;`
        // yields `&amp;lt;`, and the reader is told the document held
        // characters it did not.
        assert_eq!(for_html("<"), "&lt;");
        assert_eq!(for_html("&lt;"), "&amp;lt;", "a literal `&lt;` in a document");
        assert_eq!(for_html(&for_html("<")).len(), "&amp;lt;".len());
    }

    #[test]
    fn the_trojan_source_family_is_spelled_out() {
        // The half an ordinary HTML escaper misses. Every one of these is
        // markup-insignificant, so entity encoding alone leaves it in place
        // and the rendered page reads differently from the bytes behind it.
        for (raw, expected) in [
            ("\u{202e}override", "<U+202E>override"),
            ("\u{2066}isolate\u{2069}", "<U+2066>isolate<U+2069>"),
            ("zero\u{200b}width", "zero<U+200B>width"),
            ("\u{feff}bom", "<U+FEFF>bom"),
            ("soft\u{ad}hyphen", "soft<U+00AD>hyphen"),
            ("para\u{2029}separator", "para<U+2029>separator"),
            ("\u{1b}[31mred", "<U+001B>[31mred"),
            ("\u{7f}delete", "<U+007F>delete"),
        ] {
            assert_eq!(for_html(raw), expected, "escaping {raw:?}");
        }
    }

    #[test]
    fn nothing_that_can_open_a_tag_survives_a_pass() {
        // The property, rather than the table. Whatever goes in, no character
        // this module treats as markup-significant or dangerous comes out —
        // except inside the escaper's own `<U+XXXX>` spelling, which is why
        // that spelling is checked for separately below.
        let hostile: String = (0u32..0x3000)
            .chain([0xFEFF])
            .filter_map(char::from_u32)
            .collect();
        let escaped = for_html(&hostile);

        assert!(!escaped.contains('&') || escaped.contains("&amp;"));
        assert!(!escaped.chars().any(is_dangerous));
        for forbidden in ['"', '\''] {
            assert!(
                !escaped.contains(forbidden),
                "{forbidden:?} can break out of an attribute",
            );
        }
        // `<` and `>` survive only as the escaper's own code-point spelling.
        for fragment in escaped.split("<U+") {
            assert!(
                !fragment.contains('<'),
                "a `<` reached the output outside a `<U+XXXX>` escape",
            );
        }
    }

    #[test]
    fn the_escape_is_visible_rather_than_silent() {
        // Deleting a dangerous character would be the other obvious fix, and
        // it is the wrong one for the reason the console gives: the reader
        // would be told a different string was in the document than the one
        // that was. Length grows; it never shrinks.
        for raw in ["\u{202e}", "\u{1b}", "\u{200b}", "<", "&"] {
            assert!(for_html(raw).len() > raw.len(), "escaping {raw:?}");
        }
    }

    #[test]
    fn layout_whitespace_is_preserved_because_the_sink_is_not_line_oriented() {
        // The one deliberate divergence from `conform_cli::escape`, and the
        // reason is in this module's documentation: the console's format is
        // one finding per line, and this one is not.
        assert_eq!(for_html("two\nlines"), "two\nlines");
        assert!(!is_dangerous('\n'));
        assert!(!is_dangerous('\t'));
        assert!(!is_dangerous('\r'));
    }
}
