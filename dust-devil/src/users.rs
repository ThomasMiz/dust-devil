//! A simple user management system that stores users with username, password, and role, with the
//! ability to load and save this information from disk using a custom human-readable file format.
//!
//! Note that no information is encrypted nor hashed, passwords are stored in plain text. This is
//! not intended to be a secure system (after all, socks5 credentials are transmitted through the
//! network in plain text).
//!
//! The system is used through the `UserManager` type, which can be asynchronously created from the
//! `UserManager::from` family of methods.
//!
//! The file format used for persistence is very simple, each line consists of a user, where the
//! first character of the line specifies the role ('#' for regular users and '@' for admin),
//! followed by the username, followed by a colon ':', followed by the password until the end of
//! the line (or file).
//!
//! Characters in both the username and password may be escaped with a '\', this allows a username
//! to contain the ':' character. Any character can be escaped. A line may also be a comment by
//! starting with '!'. All lines have whitespaces trimmed at the start and empty lines are ignored.
//!
//! Note: Whitespaces aren't trimmed at the end of a line because that would prevent passwords from
//! ending with whitespaces.
//!
//! An example of a valid file is:
//! ```txt
//! ! This is a comment!
//!
//! ! Our admin Pedro, everybody loves him
//! @pedro:pedrito4321
//!
//! ! Our first user Carlos and his brother Felipe, fucken assholes
//! #carlos:carlitox@33
//! #felipe:mi_hermano_es_un_boludo
//!
//! ! My friend chi:chí, nobody knows why she put a ':' in her name:
//! #chi\:chí:super:secret:password
//! ! Chi:chí's password is "super:secret:password"
//! ```

use std::{io, path::Path};

use dashmap::{mapref::entry::Entry, DashMap};
use dust_devil_core::users::{UserRole, UsersLoadingError, ADMIN_PREFIX_CHAR, COMMENT_PREFIX_CHAR, ESCAPE_CHAR, REGULAR_PREFIX_CHAR};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufWriter},
};

use crate::utils::{self, process_lines::ProcessFileLinesError};

#[derive(Debug)]
pub struct UserManager {
    users: DashMap<String, UserData>,
}

#[derive(Debug, PartialEq)]
pub struct UserData {
    pub password: String,
    pub role: UserRole,
}

pub fn parse_line_into_user(s: &str, line_number: u32, mut char_at: u32) -> Result<Option<(String, UserData)>, UsersLoadingError> {
    let mut chars = s.chars();
    let role_char = chars
        .next()
        .ok_or(UsersLoadingError::ExpectedRoleCharGotEOF(line_number, char_at))?;
    char_at += 1;
    let role = match role_char {
        COMMENT_PREFIX_CHAR => return Ok(None),
        ADMIN_PREFIX_CHAR => UserRole::Admin,
        REGULAR_PREFIX_CHAR => UserRole::Regular,
        _ => return Err(UsersLoadingError::InvalidRoleChar(line_number, char_at, role_char)),
    };

    let mut username = String::with_capacity(255);
    let mut escape_next = false;
    loop {
        let next_char = chars.next().ok_or(UsersLoadingError::ExpectedColonGotEOF(line_number, char_at))?;
        char_at += 1;

        if escape_next || (next_char != ESCAPE_CHAR && next_char != ':') {
            if username.len() >= 255 {
                return Err(UsersLoadingError::UsernameTooLong(line_number, char_at));
            }
            username.push(next_char);
        }

        if escape_next {
            escape_next = false;
        } else if next_char == ESCAPE_CHAR {
            escape_next = true;
        } else if next_char == ':' {
            break;
        }
    }

    if username.is_empty() {
        return Err(UsersLoadingError::EmptyUsername(line_number, char_at));
    }

    let mut password = String::with_capacity(255);
    let mut escape_next = false;
    for next_char in chars {
        char_at += 1;

        if escape_next || next_char != ESCAPE_CHAR {
            if password.len() >= 255 {
                return Err(UsersLoadingError::PasswordTooLong(line_number, char_at));
            }
            password.push(next_char);
        }

        if escape_next {
            escape_next = false;
        } else if next_char == ESCAPE_CHAR {
            escape_next = true;
        }
    }

    if password.is_empty() {
        return Err(UsersLoadingError::EmptyPassword(line_number, char_at));
    }

    Ok(Some((username, UserData { password, role })))
}

impl UserManager {
    pub fn new() -> UserManager {
        UserManager { users: DashMap::new() }
    }

    pub async fn from<T>(reader: &mut T) -> Result<UserManager, UsersLoadingError>
    where
        T: AsyncRead + Unpin + ?Sized,
    {
        let users = DashMap::new();

        let result = utils::process_lines::process_lines_utf8(reader, |mut s, line_number| {
            let mut char_at = 0;
            while s.chars().next().is_some_and(|c| c.is_whitespace()) {
                s = &s[1..];
                char_at += 1;
            }

            if !s.is_empty() {
                if let Some((username, user)) = parse_line_into_user(s, line_number, char_at)? {
                    users.insert(username, user);
                }
            }

            Ok::<(), UsersLoadingError>(())
        })
        .await;

        if let Err(error) = result {
            return Err(match error {
                ProcessFileLinesError::IO(io_error) => UsersLoadingError::IO(io_error),
                ProcessFileLinesError::InvalidUtf8 { line_number, byte_at } => UsersLoadingError::InvalidUtf8 { line_number, byte_at },
                ProcessFileLinesError::LineTooLong { line_number, byte_at } => UsersLoadingError::LineTooLong { line_number, byte_at },
                ProcessFileLinesError::Cancelled(_, internal_error) => internal_error,
            });
        }

        if users.is_empty() {
            return Err(UsersLoadingError::NoUsers);
        }

        Ok(UserManager { users })
    }

    pub async fn from_file<F: AsRef<Path>>(filename: F) -> Result<UserManager, UsersLoadingError> {
        let mut file = File::open(filename).await?;
        UserManager::from(&mut file).await
    }

    pub async fn save_to<T>(&self, writer: &mut T) -> Result<(), io::Error>
    where
        T: AsyncWrite + Unpin + ?Sized,
    {
        let mut is_first = true;
        for ele in self.users.iter() {
            if !is_first {
                writer.write_u8(b'\n').await?;
            } else {
                is_first = false;
            }

            let role_char = match ele.role {
                UserRole::Admin => ADMIN_PREFIX_CHAR as u8,
                UserRole::Regular => REGULAR_PREFIX_CHAR as u8,
            };
            writer.write_u8(role_char).await?;

            for &c in ele.key().as_bytes() {
                if c == b'\\' || c == b':' {
                    writer.write_u8(b'\\').await?;
                }
                writer.write_u8(c).await?;
            }

            writer.write_u8(b':').await?;
            writer.write_all(ele.password.as_bytes()).await?;
        }

        Ok(())
    }

    pub async fn save_to_file<F: AsRef<Path>>(&self, filename: F) -> Result<(), io::Error> {
        let file = File::create(filename).await?;
        let mut writer = BufWriter::new(file);
        self.save_to(&mut writer).await?;
        writer.flush().await
    }

    pub fn insert(&self, username: String, password: String, role: UserRole) -> bool {
        // Note: This code might look like it has a race condition, as two threads could simultaneously
        // see the entry as vacant and then both try to insert the key. However, upon further examination,
        // the Entry<...> type actually holds a lock underneath, which lasts until the variable is
        // dropped! This is why the documentation for the entry function states:
        // "Locking behaviour: May deadlock if called when holding any sort of reference into the map."

        let entry = self.users.entry(username);
        if let Entry::Occupied(_) = entry {
            return false;
        }

        entry.insert(UserData { password, role });
        true
    }

    pub fn insert_or_update(&self, username: String, password: String, role: UserRole) -> bool {
        self.users.insert(username, UserData { password, role }).is_some()
    }

    pub fn count(&self) -> usize {
        self.users.len()
    }

    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
    }

    pub fn try_login(&self, username: &str, password: &str) -> Option<UserRole> {
        self.users.get(username).filter(|u| u.password == password).map(|u| u.role)
    }
}

#[cfg(test)]
mod tests {
    use dashmap::DashMap;
    use dust_devil_core::users::{UserRole, UsersLoadingError};
    use tokio::io::BufReader;

    use crate::utils::process_lines;

    use super::{UserData, UserManager};

    async fn from(s: &str) -> Result<UserManager, UsersLoadingError> {
        UserManager::from(&mut BufReader::new(s.as_bytes())).await
    }

    async fn from_bytes(s: &[u8]) -> Result<UserManager, UsersLoadingError> {
        UserManager::from(&mut BufReader::new(s)).await
    }

    fn usermap(s: &[(&str, &str, UserRole)]) -> DashMap<String, UserData> {
        let h = DashMap::new();
        for (username, password, role) in s {
            let username = String::from(*username);
            let password = String::from(*password);
            h.insert(username, UserData { password, role: *role });
        }

        h
    }

    fn assert_ok_with(result: &Result<UserManager, UsersLoadingError>, s: &[(&str, &str, UserRole)]) {
        match result {
            Ok(mgr) => {
                let expected = usermap(s);
                for ele in mgr.users.iter() {
                    assert_eq!(expected.get(ele.key()).as_deref(), Some(ele.value()));
                }

                for (username, userdata) in expected {
                    assert_eq!(mgr.users.get(&username).as_deref(), Some(&userdata));
                }
            }
            Err(_) => panic!("Expected Ok but got Err!"),
        }
    }

    fn assert_err_with(result: &Result<UserManager, UsersLoadingError>, err: UsersLoadingError) {
        match result {
            Ok(_) => panic!("Expected Err but got Ok!"),
            Err(err2) => assert_eq!(err2, &err),
        }
    }

    #[tokio::test]
    async fn test_no_users() {
        let result = from("").await;
        assert_err_with(&result, UsersLoadingError::NoUsers);

        let result = from("     ").await;
        assert_err_with(&result, UsersLoadingError::NoUsers);

        let result = from("!hola").await;
        assert_err_with(&result, UsersLoadingError::NoUsers);

        let result = from("        ! pedro 😎😎😎😎                      ").await;
        assert_err_with(&result, UsersLoadingError::NoUsers);
    }

    #[tokio::test]
    async fn test_invalid_utf8() {
        let valid = from_bytes("áeíoú💀😎🤩😪💀".as_bytes()).await;
        assert!(valid.is_err_and(|e| !matches!(
            e,
            UsersLoadingError::InvalidUtf8 {
                line_number: _,
                byte_at: _
            }
        )));

        let invalid = from_bytes(&"áeíoú💀😎🤩😪💀".as_bytes()[0..22]).await;
        assert_err_with(
            &invalid,
            UsersLoadingError::InvalidUtf8 {
                line_number: 1,
                byte_at: 21,
            },
        )
    }

    #[tokio::test]
    async fn test_line_too_long() {
        let result = from(&format!(
            "{}#pedro:pedro{}",
            " ".repeat(process_lines::BUFFER_CAPACITY - 12 - 69 - 1),
            " ".repeat(69)
        ))
        .await;
        assert_ok_with(&result, &[("pedro", &format!("pedro{}", " ".repeat(69)), UserRole::Regular)]);

        let result = from(&format!(
            "{}#pedro:pedro{}",
            " ".repeat(process_lines::BUFFER_CAPACITY - 12 - 69),
            " ".repeat(69)
        ))
        .await;
        assert_err_with(
            &result,
            UsersLoadingError::LineTooLong {
                line_number: 1,
                byte_at: process_lines::BUFFER_CAPACITY,
            },
        );
    }

    #[tokio::test]
    async fn test_invalid_rolechar() {
        let result = from("$petre:griffon").await;
        assert_err_with(&result, UsersLoadingError::InvalidRoleChar(1, 1, '$'));

        let result = from("   =").await;
        assert_err_with(&result, UsersLoadingError::InvalidRoleChar(1, 4, '='));
    }

    #[tokio::test]
    async fn test_no_password() {
        let result = from("#petre").await;
        assert_err_with(&result, UsersLoadingError::ExpectedColonGotEOF(1, 6));

        let result = from("   @sus").await;
        assert_err_with(&result, UsersLoadingError::ExpectedColonGotEOF(1, 7));
    }

    #[tokio::test]
    async fn test_empty_username() {
        let result = from("#:marcos").await;
        assert_err_with(&result, UsersLoadingError::EmptyUsername(1, 2));

        let result = from("      @:soco:troco").await;
        assert_err_with(&result, UsersLoadingError::EmptyUsername(1, 8));
    }

    #[tokio::test]
    async fn test_username_too_long() {
        let result = from(&format!(" #{}:password", "a".repeat(255))).await;
        assert_ok_with(&result, &[(&"a".repeat(255), "password", UserRole::Regular)]);

        let result = from(&format!("   #{}:password", "a".repeat(256))).await;
        assert_err_with(&result, UsersLoadingError::UsernameTooLong(1, 260));
    }

    #[tokio::test]
    async fn test_empty_password() {
        let result = from("#carmen:").await;
        assert_err_with(&result, UsersLoadingError::EmptyPassword(1, 8));

        let result = from("@chí:").await;
        assert_err_with(&result, UsersLoadingError::EmptyPassword(1, 5));
    }

    #[tokio::test]
    async fn test_password_too_long() {
        let result = from(&format!(" #username:{}", "b".repeat(255))).await;
        assert_ok_with(&result, &[("username", &"b".repeat(255), UserRole::Regular)]);

        let result = from(&format!("   #username:{}", "b".repeat(256))).await;
        assert_err_with(&result, UsersLoadingError::PasswordTooLong(1, 269));
    }

    #[tokio::test]
    async fn test_integration1() {
        let result = from(concat!(
            " ! This is a comment!\n",
            "\n",
            " ! Our admin Pedro, everybody loves him\n",
            " @pedro:pedrito4321\n",
            "\n",
            " ! Our first user Carlos and his brother Felipe, fucken assholes\n",
            " #carlos:carlitox@33\n",
            " #felipe:mi_hermano_es_un_boludo\n",
            "\n",
            " ! My friend chi:chi, nobody knows why she put a ':' in her name:\n",
            " #chi\\:chí:super:secret:password\n",
            " ! Chi:chí's password is \"super:secret:password\"\n",
        ))
        .await;

        assert_ok_with(
            &result,
            &[
                ("pedro", "pedrito4321", UserRole::Admin),
                ("carlos", "carlitox@33", UserRole::Regular),
                ("felipe", "mi_hermano_es_un_boludo", UserRole::Regular),
                ("chi:chí", "super:secret:password", UserRole::Regular),
            ],
        );
    }
}
