//! Provides the [`U8ReprEnum`] trait, which is made to be implemented by enums that can be
//! converted into or parsed from an [`u8`] value, for easy serialization and deserialization.

/// Allows a type to be converted into or parsed from an [`u8`] representation.
pub trait U8ReprEnum: Sized {
    /// Parses an `u8` into the enum variant it represents. If the `u8` represents a variant in
    /// this enum, then `Some` is returned with said variant. Otherwise, `None` is returned.
    fn from_u8(value: u8) -> Option<Self>;

    /// Converts this enum into its `u8` representation.
    fn into_u8(self) -> u8;
}
