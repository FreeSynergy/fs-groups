//! Text rules for ids that decide who is admin: one way to compare, one way to refuse.
//!
//! Pattern: Specification (pure) — a role id or an identity is compared in its folded form, and text
//! with control or invisible characters is refused at the input, so a spelling variant of an admin
//! role cannot pass as another role. A role id that is not in the role registry stays an opaque
//! string; only a variant of a configured admin role is treated as that role.

use unicode_normalization::UnicodeNormalization;

use crate::error::{Error, Result};

/// Invisible format characters (Unicode category Cf and similar) that look like nothing.
fn invisible(c: char) -> bool {
    matches!(c,
        '\u{ad}' | '\u{34f}' | '\u{600}'..='\u{605}' | '\u{61c}' | '\u{6dd}' | '\u{70f}' | '\u{115f}'
        | '\u{1160}' | '\u{17b4}' | '\u{17b5}' | '\u{180b}'..='\u{180e}' | '\u{200b}'..='\u{200f}'
        | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{3164}' | '\u{fe00}'..='\u{fe0f}'
        | '\u{feff}' | '\u{ffa0}' | '\u{fff9}'..='\u{fffb}' | '\u{e0000}'..='\u{e0fff}')
}

/// Refuse control and invisible characters in `s` (`what` names it in the error).
pub(crate) fn check(what: &str, s: &str) -> Result<()> {
    match s.chars().find(|c| c.is_control() || invisible(*c)) {
        Some(c) => Err(Error::Conflict(format!(
            "{what} contains a control or invisible character (U+{:04X})",
            u32::from(c)
        ))),
        None => Ok(()),
    }
}

/// The folded form of a role id: trimmed, compatibility-normalised, case folded.
pub(crate) fn fold(s: &str) -> String {
    let lower: String = s.trim().nfkc().collect::<String>().to_lowercase();
    lower.replace('ß', "ss").nfkc().collect()
}

/// An identity as it is asked of the host: trimmed, scheme and host in lower case, no trailing
/// slash. The path keeps its case. An id that still differs is unknown to the host, and unknown
/// means "not known to be human".
pub(crate) fn canonical_person(s: &str) -> String {
    let s = s.trim();
    let Some((scheme, rest)) = s.split_once("://") else {
        return s.to_string();
    };
    let (authority, path) = rest.find('/').map_or((rest, ""), |i| rest.split_at(i));
    format!(
        "{}://{}{}",
        scheme.to_lowercase(),
        authority.to_lowercase(),
        path.trim_end_matches('/')
    )
}
