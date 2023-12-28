use dashmap::DashMap;
use dust_devil_core::users::{UserRole, UsersLoadingError};
use tokio::io::BufReader;

use crate::utils::process_lines;

use crate::users::{UserData, UserManager};

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
            for ele in mgr.users().iter() {
                assert_eq!(expected.get(ele.key()).as_deref(), Some(ele.value()));
            }

            for (username, userdata) in expected {
                assert_eq!(mgr.users().get(&username).as_deref(), Some(&userdata));
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
