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
        let borrow = self.user_auth.read();
        match AuthRefreshRequest::new(
            borrow.uid.expose_secret(),
            borrow.refresh_token.expose_secret(),
        )
        .execute_sync(client, &DefaultRequestFactory {})
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

    async fn refresh_auth_async<C: ClientAsync>(&self, client: &C) -> http::Result<()> {
        // Have to clone here due to async boundaries.
        let user_auth = self.user_auth.read().clone();
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
                    } else if api_err.http_code == 422 && api_err.http_code == 9001 {
                        //TODO: Handle captcha .....
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
