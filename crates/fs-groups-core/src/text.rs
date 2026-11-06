//! Text rules for ids that decide who is admin: one way to compare, one way to refuse.
//!
//! Pattern: Specification (pure) — a role id or an identity is compared in its folded form, and text
//! with control or invisible characters is refused at the input, so a spelling variant of an admin
//! role cannot pass as another role. A role id that is not in the role registry stays an opaque
//! string; only a variant of a configured admin role is treated as that role.

use icu_properties::props::{DefaultIgnorableCodePoint, GeneralCategory, GeneralCategoryGroup};
use icu_properties::{CodePointMapData, CodePointSetData};
use unicode_normalization::UnicodeNormalization;

use crate::error::{Error, Result};

/// Punctuation a role id may contain besides ASCII letters and digits.
const ROLE_PUNCT: &str = "-_.:/#@+~%=";

fn refuse(what: &str, c: char) -> Error {
    Error::Conflict(format!(
        "{what} contains a character that is not allowed (U+{:04X})",
        u32::from(c)
    ))
}

/// A role id is ASCII only: letters, digits and a few punctuation marks. That refuses control,
/// invisible and look-alike characters by shape, instead of by a list of the ones we know of.
/// (fs-people compares role ids exactly; a role id with a letter outside ASCII is refused here.)
pub(crate) fn check_role(what: &str, s: &str) -> Result<()> {
    match s
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || ROLE_PUNCT.contains(*c)))
    {
        Some(c) => Err(refuse(what, c)),
        None => Ok(()),
    }
}

/// Punctuation an identity may contain besides letters, digits and marks: the ASCII punctuation of an
/// IRI and of an e-mail address (the caret, braces, bar, backtick and quotes of an e-mail local part
/// are allowed).
const IDENTITY_PUNCT: &str = "-._~:/?#[]@!$&'()*+,;=%^{|}`\"";

/// An identity (an IRI, or an e-mail address), checked and in Unicode normal form C, so that a
/// decomposed and a composed spelling of the same name are one person.
///
/// Allowed: letters and digits of any script, combining marks (a virama, a diaeresis), the ASCII
/// punctuation above, and a zero width (non-)joiner between letters or after a mark, where scripts
/// need it (Persian, Devanagari). Refused: whitespace, controls, every other invisible or format
/// character (default ignorable code points such as the Hangul fillers, bidi controls, variation
/// selectors, tags), a joiner at an end or between non-letters, and unassigned code points. Look-alike
/// letters of other scripts are not told apart; that is for the host's identity rules.
pub(crate) fn person(what: &str, s: &str) -> Result<String> {
    let s: String = s.trim().nfc().collect();
    let ignorable = CodePointSetData::new::<DefaultIgnorableCodePoint>();
    let categories = CodePointMapData::<GeneralCategory>::new();
    let chars: Vec<char> = s.chars().collect();
    let is_mark = |c: char| GeneralCategoryGroup::Mark.contains(categories.get(c));
    for (i, c) in chars.iter().copied().enumerate() {
        let joiner = c == '\u{200c}' || c == '\u{200d}';
        let ok = if joiner {
            let before = i > 0 && (chars[i - 1].is_alphabetic() || is_mark(chars[i - 1]));
            let after = chars.get(i + 1).is_some_and(|n| n.is_alphabetic());
            before && after
        } else {
            !ignorable.contains(c)
                && (c.is_alphanumeric() || is_mark(c) || IDENTITY_PUNCT.contains(c))
        };
        if !ok {
            return Err(refuse(what, c));
        }
    }
    Ok(s)
}

/// The folded form of a role id: trimmed, compatibility-normalised, case folded.
pub(crate) fn fold(s: &str) -> String {
    let lower: String = s.trim().nfkc().collect::<String>().to_lowercase();
    lower.replace('ß', "ss").nfkc().collect()
}

/// An identity as it is asked of the host: trimmed, the scheme and the host in lower case, nothing
/// else changed. The user part (before the last `@` of the authority), the path, the query and the
/// fragment keep their case, and a trailing slash stays: they are different identities. An id that
/// still differs from the host's is unknown to it, and unknown means "not known to be human".
pub(crate) fn canonical_person(s: &str) -> String {
    let s = s.trim();
    let Some((scheme, rest)) = s.split_once("://") else {
        return s.to_string();
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let (user, host) = authority
        .rfind('@')
        .map_or(("", authority), |i| authority.split_at(i + 1));
    format!(
        "{}://{user}{}{tail}",
        scheme.to_lowercase(),
        host.to_lowercase()
    )
}
