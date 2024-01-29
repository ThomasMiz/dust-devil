//! Provides the [`parse_pretty_buffer_size`] function, used to parse human-readable byte counts.

use std::num::IntErrorKind;

/// An error from parsing an invalid string with [`parse_pretty_buffer_size`].
pub enum PrettyBufferSizeParseError {
    /// The string is empty or blank.
    Empty,

    /// The string represents the value 0.
    Zero,

    /// The string doesn't follow a valid format.
    InvalidFormat,

    /// The string contains invalid characters.
    InvalidCharacters,

    /// The string represents a value equal to or over the 4GB limit.
    TooLarge,
}

/// Parses a human-readable byte count that is not zero and not greater than [`u32::MAX`].
///
/// If a user wanted to specify a buffer size of 3 megabytes, then it'd be a pain in the ass for
/// them to have to calculate and write `3145728`. So instead, this function allows parsing pretty
/// values, like `3072K` or `3M` (or `3072KB` or `3MB`).
///
/// The available suffixes are `K` (kilo), `M` (mega) and `G` (giga). They are case-insensitive and may
/// optionally be followed by a `b`/`B` character. Numbers can be specified in hexadecimal,
/// octal or binary by using the (also case-insensitive) prefixes `0x`, `0o` and `0b` respectively.
///
/// Decimal numbers (e.g. `1.5M` or `16.0KB`) are not allowed.
pub fn parse_pretty_buffer_size(s: &str) -> Result<u32, PrettyBufferSizeParseError> {
    let s = s.trim();

    if s.is_empty() {
        return Err(PrettyBufferSizeParseError::Empty);
    }

    let mut iter = s.chars();
    let (s, radix) = match (iter.next(), iter.next().map(|c| c.to_ascii_lowercase())) {
        (Some('0'), Some('x')) => (&s[2..], 16),
        (Some('0'), Some('o')) => (&s[2..], 8),
        (Some('0'), Some('b')) => (&s[2..], 2),
        _ => (s, 10),
    };

    let mut iter = s.chars();
    let (s, multiplier) = match iter.next_back().map(|c| c.to_ascii_lowercase()) {
        Some('k') => (&s[..(s.len() - 1)], 1024),
        Some('m') => (&s[..(s.len() - 1)], 1024 * 1024),
        Some('g') => (&s[..(s.len() - 1)], 1024 * 1024 * 1024),
        Some('b') => match iter.next_back().map(|c| c.to_ascii_lowercase()) {
            Some('k') => (&s[..(s.len() - 2)], 1024),
            Some('m') => (&s[..(s.len() - 2)], 1024 * 1024),
            Some('g') => (&s[..(s.len() - 2)], 1024 * 1024 * 1024),
            Some(_) if radix < 11 => (&s[..(s.len() - 1)], 1),
            _ => (s, 1),
        },
        _ => (s, 1),
    };

    match s.chars().next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return Err(PrettyBufferSizeParseError::InvalidFormat),
    }

    let size = match u32::from_str_radix(s, radix) {
        Ok(0) => return Err(PrettyBufferSizeParseError::Zero),
        Ok(size) => size,
        Err(parse_int_error) => {
            return Err(match parse_int_error.kind() {
                IntErrorKind::Empty => PrettyBufferSizeParseError::Empty,
                IntErrorKind::PosOverflow => PrettyBufferSizeParseError::TooLarge,
                _ => PrettyBufferSizeParseError::InvalidCharacters,
            });
        }
    };

    match size.checked_mul(multiplier) {
        Some(size) => Ok(size),
        None => Err(PrettyBufferSizeParseError::TooLarge),
    }
}
