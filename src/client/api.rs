use crate::client::types::{LatestEventResponse, UserAuth};
use crate::client::{HttpClient, RequestBuilder, X_PM_UID_HEADER};
use crate::domain::{Event, EventId, SecretString, User, UserUid};
use crate::RequestError;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;

/// An authenticated REST API Client to access the proton REST API services.
#[derive(Debug)]
pub struct Client {
    pub(crate) http_client: HttpClient,
    pub(crate) user: UserAuth,
}

impl Client {
    /// Get the currently logged in user's UID.
    pub fn user_uid(&self) -> &Secret<UserUid> {
        &self.user.uid
    }

    /// Get the currently logged in user's refresh token.
    pub fn user_refresh_token(&self) -> &SecretString {
        &self.user.refresh_token
    }

    /// Logout the current user. Consumes the type in the process. If the request fails, the
    /// current instance is returned in the Error.
    pub async fn logout(self) -> Result<(), (Self, RequestError)> {
        if let Err(e) = self.delete("/auth/v4").execute().await {
            return Err((self, e));
        }

        Ok(())
    }

    /// Get the currently logged in user's information.
    pub async fn get_user(&self) -> Result<User, RequestError> {
        let response = self.get("/core/v4/users").execute().await?;

        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct UserResponse {
            user: User,
        }

        let user = response.json::<UserResponse>().await?;

        Ok(user.user)
    }

    /// Get the latest event id for the logged in user. This is meant to be used to initialize
    /// the even update loop.
    pub async fn get_latest_event_id(&self) -> Result<EventId, RequestError> {
        let response = self.get("/core/v4/events/latest").execute().await?;
        let latest = response.json::<LatestEventResponse>().await?;
        Ok(latest.event_id)
    }

    /// Get an event based on the given `event_id`.
    pub async fn get_event(&self, event_id: &EventId) -> Result<Event, RequestError> {
        let response = self
            .get(&format!("/core/v4/events/{event_id}"))
            .execute()
            .await?;

        let event = response.json::<Event>().await?;
        Ok(event)
    }

    /// Create a new post request.
    pub(crate) fn post(&self, url: &str) -> RequestBuilder {
        self.append_headers(self.http_client.post(url))
    }

    /// Create a new get request.
    pub(crate) fn get(&self, url: &str) -> RequestBuilder {
        self.append_headers(self.http_client.get(url))
    }

    /// Create a new delete request.
    pub(crate) fn delete(&self, url: &str) -> RequestBuilder {
        self.append_headers(self.http_client.delete(url))
    }

    #[inline(always)]
    fn append_headers(&self, builder: RequestBuilder) -> RequestBuilder {
        builder
            .header(X_PM_UID_HEADER, &self.user.uid.expose_secret().0)
            .bearer_token(self.user.access_token.expose_secret())
    }
}
