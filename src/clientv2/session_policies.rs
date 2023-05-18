//! Session Request Policies allow us to inject certain behaviour into the execution of the requests
//! such as auto refreshing account token on session expiry.

use crate::http::{
    ClientAsync, ClientSync, DefaultRequestFactory, Method, Request, RequestData, RequestFactory,
};
use crate::requests::{AuthRefreshRequest, UserAuth};
use crate::{
    http, Session, SessionRefreshData, SessionRequestPolicy, UserAuthFromSessionRequestPolicy,
};
use secrecy::ExposeSecret;
use std::future::Future;
use std::pin::Pin;

/// Default session request policy, does nothing special.
#[derive(Debug)]
pub struct DefaultSessionRequestPolicy {
    user: UserAuth,
}

impl From<UserAuth> for DefaultSessionRequestPolicy {
    fn from(value: UserAuth) -> Self {
        Self { user: value }
    }
}

impl RequestFactory for DefaultSessionRequestPolicy {
    fn new_request(&self, method: Method, url: &str) -> RequestData {
        RequestData::new(method, url)
            .header(http::X_PM_UID_HEADER, &self.user.uid.expose_secret().0)
            .bearer_token(self.user.access_token.expose_secret())
    }
}

impl SessionRequestPolicy for DefaultSessionRequestPolicy {
    fn execute<C: ClientSync, R: Request>(
        &self,
        client: &C,
        request: R,
    ) -> http::Result<R::Output> {
        request.execute_sync(client, self)
    }

    fn execute_async<'a, C: ClientAsync, R: Request + 'a>(
        &'a self,
        client: &'a C,
        request: R,
    ) -> Pin<Box<dyn Future<Output = http::Result<R::Output>> + 'a>> {
        request.execute_async(client, self)
    }

    fn get_refresh_data(&self) -> SessionRefreshData {
        SessionRefreshData {
            user_uid: self.user.uid.clone(),
            token: self.user.refresh_token.clone(),
        }
    }
}

impl UserAuthFromSessionRequestPolicy for DefaultSessionRequestPolicy {
    fn user_auth(&self) -> &UserAuth {
        &self.user
    }

    fn user_auth_mut(&mut self) -> &mut UserAuth {
        &mut self.user
    }
}

/// This session policy will attempt to refresh the session token if the session expires.
#[derive(Debug)]
struct AutoAuthRefresherRequestPolicyInner<
    T: SessionRequestPolicy + UserAuthFromSessionRequestPolicy,
> {
    policy: T,
    was_refreshed: bool,
}

#[derive(Debug)]
pub struct AutoAuthRefreshRequestPolicy<T: SessionRequestPolicy + UserAuthFromSessionRequestPolicy>(
    parking_lot::RwLock<AutoAuthRefresherRequestPolicyInner<T>>,
);

impl<T: SessionRequestPolicy + UserAuthFromSessionRequestPolicy> From<UserAuth>
    for AutoAuthRefreshRequestPolicy<T>
{
    fn from(value: UserAuth) -> Self {
        Self(parking_lot::RwLock::new(
            AutoAuthRefresherRequestPolicyInner {
                policy: T::from(value),
                was_refreshed: false,
            },
        ))
    }
}

impl<T: SessionRequestPolicy + UserAuthFromSessionRequestPolicy> RequestFactory
    for AutoAuthRefreshRequestPolicy<T>
{
    fn new_request(&self, method: Method, url: &str) -> RequestData {
        self.0.read().policy.new_request(method, url)
    }
}

impl<T: SessionRequestPolicy + UserAuthFromSessionRequestPolicy> AutoAuthRefreshRequestPolicy<T> {
    fn refresh_auth<C: ClientSync>(&self, client: &C) -> http::Result<()> {
        let mut borrow = self.0.write();
        borrow.was_refreshed = false;
        match AuthRefreshRequest::new(
            borrow.policy.user_auth().uid.expose_secret(),
            borrow.policy.user_auth().refresh_token.expose_secret(),
        )
        .execute_sync(client, &DefaultRequestFactory {})
        {
            Ok(s) => {
                *borrow.policy.user_auth_mut() = UserAuth::from_auth_refresh_response(&s);
                borrow.was_refreshed = true;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn refresh_auth_async<C: ClientAsync>(&self, client: &C) -> http::Result<()> {
        let user_auth = {
            let mut borrow = self.0.write();
            borrow.was_refreshed = false;
            borrow.policy.user_auth().clone()
        };
        match AuthRefreshRequest::new(
            user_auth.uid.expose_secret(),
            user_auth.refresh_token.expose_secret(),
        )
        .execute_async(client, &DefaultRequestFactory {})
        .await
        {
            Ok(s) => {
                let mut borrow = self.0.write();
                *borrow.policy.user_auth_mut() = UserAuth::from_auth_refresh_response(&s);
                borrow.was_refreshed = true;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub(super) fn was_auth_refreshed(&self) -> bool {
        self.0.read().was_refreshed
    }
}

impl<T: SessionRequestPolicy + UserAuthFromSessionRequestPolicy> SessionRequestPolicy
    for AutoAuthRefreshRequestPolicy<T>
{
    fn execute<C: ClientSync, R: Request>(
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

    fn execute_async<'a, C: ClientAsync, R: Request + 'a>(
        &'a self,
        client: &'a C,
        request: R,
    ) -> Pin<Box<dyn Future<Output = http::Result<R::Output>> + 'a>> {
        Box::pin(async move {
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
        })
    }

    fn get_refresh_data(&self) -> SessionRefreshData {
        self.0.read().policy.get_refresh_data()
    }
}

pub type DefaultSession = Session<DefaultSessionRequestPolicy>;
pub type AutoRefreshAuthSession =
    Session<AutoAuthRefreshRequestPolicy<DefaultSessionRequestPolicy>>;
