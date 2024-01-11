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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SandstormCommandType {
    Shutdown = 0x00,
    LogEventConfig = 0x01,
    LogEventStream = 0x02,
    ListSocks5Sockets = 0x03,
    AddSocks5Socket = 0x04,
    RemoveSocks5Socket = 0x05,
    ListSandstormSockets = 0x06,
    AddSandstormSocket = 0x07,
    RemoveSandstormSocket = 0x08,
    ListUsers = 0x09,
    AddUser = 0x0A,
    UpdateUser = 0x0B,
    DeleteUser = 0x0C,
    ListAuthMethods = 0x0D,
    ToggleAuthMethod = 0x0E,
    RequestCurrentMetrics = 0x0F,
    GetBufferSize = 0x10,
    SetBufferSize = 0x11,
    Meow = 0xFF,
}

impl SandstormCommandType {
    pub fn from_u8(value: u8) -> Option<SandstormCommandType> {
        match value {
            0x00 => Some(Self::Shutdown),
            0x01 => Some(Self::LogEventConfig),
            0x02 => Some(Self::LogEventStream),
            0x03 => Some(Self::ListSocks5Sockets),
            0x04 => Some(Self::AddSocks5Socket),
            0x05 => Some(Self::RemoveSocks5Socket),
            0x06 => Some(Self::ListSandstormSockets),
            0x07 => Some(Self::AddSandstormSocket),
            0x08 => Some(Self::RemoveSandstormSocket),
            0x09 => Some(Self::ListUsers),
            0x0A => Some(Self::AddUser),
            0x0B => Some(Self::UpdateUser),
            0x0C => Some(Self::DeleteUser),
            0x0D => Some(Self::ListAuthMethods),
            0x0E => Some(Self::ToggleAuthMethod),
            0x0F => Some(Self::RequestCurrentMetrics),
            0x10 => Some(Self::GetBufferSize),
            0x11 => Some(Self::SetBufferSize),
            0xFF => Some(Self::Meow),
            _ => None,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AddUserResponse {
    Ok = 0,
    AlreadyExists = 1,
    InvalidValues = 2,
}

impl AddUserResponse {
    pub fn from_u8(value: u8) -> Option<AddUserResponse> {
        match value {
            0 => Some(AddUserResponse::Ok),
            1 => Some(AddUserResponse::AlreadyExists),
            2 => Some(AddUserResponse::InvalidValues),
            _ => None,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpdateUserResponse {
    Ok = 0,
    UserNotFound = 1,
    CannotRemoveOnlyAdmin = 2,
    NothingWasRequested = 3,
}

impl UpdateUserResponse {
    pub fn from_u8(value: u8) -> Option<UpdateUserResponse> {
        match value {
            0 => Some(UpdateUserResponse::Ok),
            1 => Some(UpdateUserResponse::UserNotFound),
            2 => Some(UpdateUserResponse::CannotRemoveOnlyAdmin),
            3 => Some(UpdateUserResponse::NothingWasRequested),
            _ => None,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeleteUserResponse {
    Ok = 0,
    UserNotFound = 1,
    CannotRemoveOnlyAdmin = 2,
}

impl DeleteUserResponse {
    pub fn from_u8(value: u8) -> Option<DeleteUserResponse> {
        match value {
            0 => Some(DeleteUserResponse::Ok),
            1 => Some(DeleteUserResponse::UserNotFound),
            2 => Some(DeleteUserResponse::CannotRemoveOnlyAdmin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub current_client_connections: u32,
    pub historic_client_connections: u64,
    pub client_bytes_sent: u64,
    pub client_bytes_received: u64,
    pub current_sandstorm_connections: u32,
    pub historic_sandstorm_connections: u64,
}
