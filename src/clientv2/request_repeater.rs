//! Automatic request repeater based on the expectations Proton has for their clients.

use crate::domain::{SecretString, UserUid};
use crate::http::{
    ClientAsync, ClientSync, DefaultRequestFactory, Method, Request, RequestData, RequestFactory,
};
use crate::requests::{AuthRefreshRequest, UserAuth};
use crate::{http, SessionRefreshData};
use secrecy::{ExposeSecret, Secret};

pub trait OnAuthRefreshed: Send + Sync {
    fn on_auth_refreshed(&self, user: &Secret<UserUid>, token: &SecretString);
}

pub struct RequestRepeater {
    user_auth: parking_lot::RwLock<UserAuth>,
    on_auth_refreshed: Option<Box<dyn OnAuthRefreshed>>,
}

impl std::fmt::Debug for RequestRepeater {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RequestRepeater{{user_auth:{:?} on_auth_refreshed:{}}}",
            self.user_auth,
            if self.on_auth_refreshed.is_some() {
                "Some"
            } else {
                "None"
            }
        )
    }
}

impl RequestRepeater {
    pub fn new(user_auth: UserAuth, on_auth_refreshed: Option<Box<dyn OnAuthRefreshed>>) -> Self {
        Self {
            user_auth: parking_lot::RwLock::new(user_auth),
            on_auth_refreshed,
        }
    }

    fn refresh_auth<C: ClientSync>(&self, client: &C) -> http::Result<()> {
        let mut borrow = self.user_auth.write();
        match AuthRefreshRequest::new(
            borrow.uid.expose_secret(),
            borrow.refresh_token.expose_secret(),
        )
        .execute_sync(client, &DefaultRequestFactory {})
        {
            Ok(s) => {
                *borrow = UserAuth::from_auth_refresh_response(&s);
                if let Some(cb) = &self.on_auth_refreshed {
                    cb.on_auth_refreshed(&borrow.uid, &borrow.access_token);
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn refresh_auth_async<C: ClientAsync>(&self, client: &C) -> http::Result<()> {
        // Have to clone here due to async boundaries.
        let user_auth = { self.user_auth.read().clone() };
        match AuthRefreshRequest::new(
            user_auth.uid.expose_secret(),
            user_auth.refresh_token.expose_secret(),
        )
        .execute_async(client, &DefaultRequestFactory {})
        .await
        {
            Ok(s) => {
                let mut borrow = self.user_auth.write();
                *borrow = UserAuth::from_auth_refresh_response(&s);
                if let Some(cb) = &self.on_auth_refreshed {
                    cb.on_auth_refreshed(&borrow.uid, &borrow.access_token);
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn execute<C: ClientSync, R: Request>(
        &self,
        client: &C,
        request: R,
    ) -> http::Result<R::Output> {
        match request.execute_sync(client, self) {
            Ok(r) => Ok(r),
            Err(original_error) => {
                if let http::Error::API(api_err) = &original_error {
                    if api_err.http_code == 401 {
                        log::debug!("Account session expired, attempting refresh");
                        // Session expired/not authorized, try auth refresh.
                        if let Err(e) = self.refresh_auth(client) {
                            log::error!("Failed to refresh account {e}");
                            return Err(original_error);
                        }

                        // Execute request again
                        return request.execute_sync(client, self);
                    }
                }
                Err(original_error)
            }
        }
    }

    pub async fn execute_async<'a, C: ClientAsync, R: Request + 'a>(
        &'a self,
        client: &'a C,
        request: R,
    ) -> http::Result<R::Output> {
        match request.execute_async(client, self).await {
            Ok(r) => Ok(r),
            Err(original_error) => {
                if let http::Error::API(api_err) = &original_error {
                    log::debug!("Account session expired, attempting refresh");
                    if api_err.http_code == 401 {
                        // Session expired/not authorized, try auth refresh.
                        if let Err(e) = self.refresh_auth_async(client).await {
                            log::error!("Failed to refresh account {e}");
                            return Err(original_error);
                        }

                        // Execute request again
                        return request.execute_async(client, self).await;
                    }
                }
                Err(original_error)
            }
        }
    }

    pub fn get_refresh_data(&self) -> SessionRefreshData {
        let borrow = self.user_auth.read();
        SessionRefreshData {
            user_uid: borrow.uid.clone(),
            token: borrow.refresh_token.clone(),
        }
    }
}

impl RequestFactory for RequestRepeater {
    fn new_request(&self, method: Method, url: &str) -> RequestData {
        let accessor = self.user_auth.read();
        RequestData::new(method, url)
            .header(http::X_PM_UID_HEADER, &accessor.uid.expose_secret().0)
            .bearer_token(accessor.access_token.expose_secret())
    }
}

#[cfg(test)]
mod test {

    #[test]
    #[cfg(feature = "http-ureq")]
    fn request_repeats_with_401() {
        use crate::domain::{EventId, SecretString, UserUid};
        use crate::http::X_PM_UID_HEADER;
        use crate::requests::{GetLatestEventRequest, UserAuth};
        use crate::RequestRepeater;
        use httpmock::prelude::*;
        use secrecy::Secret;

        let server = MockServer::start();
        let url = server.base_url();

        let client = crate::http::ClientBuilder::new()
            .base_url(&url)
            .build::<crate::http::ureq_client::UReqClient>()
            .unwrap();

        let repeater = RequestRepeater::new(
            UserAuth {
                uid: Secret::new(UserUid("test-uid".to_string())),
                access_token: SecretString::new("secret-token".to_string()),
                refresh_token: SecretString::new("refresh-token".to_string()),
            },
            None,
        );

        let expected_latest_event_id = EventId("My_Event_Id".to_string());

        let latest_event_first_call = server.mock(|when, then| {
            when.method(GET)
                .path("/core/v4/events/latest")
                .header(X_PM_UID_HEADER, "test-uid");
            then.status(401);
        });

        let latest_event_second_call = server.mock(|when, then| {
            when.method(GET)
                .path("/core/v4/events/latest")
                .header(X_PM_UID_HEADER, "User_UID");
            then.status(200)
                .body(format!(r#"{{"EventID":"{}"}}"#, expected_latest_event_id.0));
        });

        let refresh_mock = server.mock(|when, then| {
            when.method(POST).path("/auth/v4/refresh");

            let response = r#"{
    "UID": "User_UID",
    "TokenType": "type",
    "AccessToken": "access-token",
    "RefreshToken": "refresh-token",
    "Scope": "Scope"
}"#;

            then.status(200).body(response);
        });

        let latest_event = repeater.execute(&client, GetLatestEventRequest {}).unwrap();
        assert_eq!(latest_event.event_id, expected_latest_event_id);

        latest_event_first_call.assert();
        refresh_mock.assert();
        latest_event_second_call.assert();
    }
}
