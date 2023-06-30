use crate::utils::create_session_and_server;
use proton_api_rs::{http, LoginError, Session, SessionType};

const DEFAULT_USER_EMAIL: &str = "foo@bar.com";
const DEFAULT_USER_PASSWORD: &str = "12345";

#[test]
fn session_login() {
    let (client, server) = create_session_and_server();
    let (user_id, _) = server
        .create_user(DEFAULT_USER_EMAIL, DEFAULT_USER_PASSWORD)
        .expect("failed to create default user");
    let auth_result = Session::login(
        &client,
        DEFAULT_USER_EMAIL,
        DEFAULT_USER_PASSWORD,
        None,
        None,
    )
    .expect("Failed to login");

    assert!(matches!(auth_result, SessionType::Authenticated(_)));

    if let SessionType::Authenticated(s) = auth_result {
        let user = s.get_user(&client).expect("Failed to get user");
        assert_eq!(user.id.as_ref(), user_id.as_ref());

        s.logout(&client).expect("Failed to logout")
    }
}

#[test]
fn session_login_invalid_user() {
    let (client, _server) = create_session_and_server();
    let auth_result = Session::login(&client, "bar", DEFAULT_USER_PASSWORD, None, None);

    assert!(matches!(
        auth_result,
        Err(LoginError::Request(http::Error::API(_)))
    ));
}
