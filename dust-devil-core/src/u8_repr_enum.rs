pub trait U8ReprEnum: Sized {
    fn from_u8(value: u8) -> Option<Self>;
    fn into_u8(self) -> u8;
}
