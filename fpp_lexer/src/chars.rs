//! Constants and classifier methods for characters

/// Line feed
pub const LF: char = '\u{000A}';

/// Carriage return
pub const CR: char = '\u{000D}';

/// Convert a character digit to an integer according to the given base,
/// `None` if no success
pub fn digit2int(ch: char, base: u32) -> Option<u32> {
    let num = match ch {
        '0'..='9' => ch as u32 - '0' as u32,
        'a'..='z' => ch as u32 - 'a' as u32 + 10,
        'A'..='Z' => ch as u32 - 'A' as u32 + 10,
        _ => return None,
    };

    if num < base { Some(num) } else { None }
}

/// Whether `c` may be part of a Unicode identifier.
///
/// The test is the one Java's `Character.isUnicodeIdentifierPart` makes for
/// a single UTF-16 code unit: an identifier-continue character or an
/// identifier-ignorable character of the Basic Multilingual Plane. A
/// character outside that plane is two surrogate code units, neither of which
/// is part of an identifier.
pub fn is_identifier_part(c: char) -> bool {
    c <= '\u{FFFF}' && (unicode_ident::is_xid_continue(c) || is_identifier_ignorable(c))
}

/// Whether `c` is ignorable in an identifier: a control character that is not
/// whitespace, or a format character of the Basic Multilingual Plane
fn is_identifier_ignorable(c: char) -> bool {
    matches!(
        c,
        '\u{0000}'..='\u{0008}'
            | '\u{000E}'..='\u{001B}'
            | '\u{007F}'..='\u{009F}'
            // Format characters
            | '\u{00AD}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061C}'
            | '\u{06DD}'
            | '\u{070F}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08E2}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
    )
}
