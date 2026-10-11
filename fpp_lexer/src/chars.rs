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

/// Whether `c` may be part of a Unicode identifier
pub fn is_identifier_part(c: char) -> bool {
    unicode_ident::is_xid_continue(c)
}
