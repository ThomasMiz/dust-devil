#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SandstormHandshakeStatus {
    Ok = 0x00,
    UnsupportedVersion = 0x01,
    InvalidUsernameOrPassword = 0x02,
    PermissionDenied = 0x03,
    // UnspecifiedError = 0xFF,
}

impl SandstormHandshakeStatus {
    pub fn from_u8(value: u8) -> Option<SandstormHandshakeStatus> {
        match value {
            0x00 => Some(SandstormHandshakeStatus::Ok),
            0x01 => Some(SandstormHandshakeStatus::UnsupportedVersion),
            0x02 => Some(SandstormHandshakeStatus::InvalidUsernameOrPassword),
            0x03 => Some(SandstormHandshakeStatus::PermissionDenied),
            _ => None,
        }
    }
}
