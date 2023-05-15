use crate::clientv2::TotpSession;
use crate::domain::{Event, EventId, TwoFactorAuth, User, UserUid};
use crate::http;
use crate::http::{DefaultRequestFactory, RequestFactory, RequestNoBody, RequestWithBody};
use crate::requests::{
    AuthInfoRequest, AuthInfoResponse, AuthRefreshRequest, AuthRequest, AuthResponse,
    GetEventRequest, GetLatestEventRequest, LogoutRequest, TFAStatus, UserAuth, UserInfoRequest,
};
use go_srp::SRPAuth;
use secrecy::ExposeSecret;

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("{0}")]
    Request(
        #[from]
        #[source]
        http::Error,
    ),
    #[error("Server SRP proof verification failed: {0}")]
    ServerProof(String),
    #[error("Account 2FA method ({0})is not supported")]
    Unsupported2FA(TwoFactorAuth),
    #[error("Failed to calculate SRP Proof: {0}")]
    SRPProof(String),
}

#[derive(Debug)]
pub enum SessionType {
    Authenticated(Session),
    AwaitingTotp(TotpSession),
}

#[derive(Debug)]
pub struct Session {
    user: UserAuth,
}

impl Session {
    fn new(user: UserAuth) -> Self {
        Self { user }
    }

    fn apply_auth_token(&self, request: http::Request) -> http::Request {
        request
            .header(http::X_PM_UID_HEADER, &self.user.uid.expose_secret().0)
            .bearer_token(self.user.access_token.expose_secret())
    }

    fn new_request(&self, method: http::Method, url: &str) -> http::Request {
        let request = http::Request::new(method, url);
        self.apply_auth_token(request)
    }
}

impl Session {
    pub fn login<T: http::ClientSync>(
        client: &T,
        username: &str,
        password: &str,
    ) -> Result<SessionType, LoginError> {
        let auth_info_response =
            AuthInfoRequest { username }.execute_sync::<T>(client, &DefaultRequestFactory {})?;

        let proof = generate_session_proof(username, password, &auth_info_response)?;

        let auth_response = AuthRequest {
            username,
            client_ephemeral: &proof.client_ephemeral,
            client_proof: &proof.client_proof,
            srp_session: auth_info_response.srp_session.as_ref(),
        }
        .execute_sync::<T>(client, &DefaultRequestFactory {})?;

        validate_server_proof(&proof, &auth_response)
    }

    pub async fn login_async<T: http::ClientAsync>(
        client: &T,
        username: &str,
        password: &str,
    ) -> Result<SessionType, LoginError> {
        let auth_info_response = AuthInfoRequest { username }
            .execute_async::<T>(client, &DefaultRequestFactory {})
            .await?;

        let proof = generate_session_proof(username, password, &auth_info_response)?;

        let auth_response = AuthRequest {
            username,
            client_ephemeral: &proof.client_ephemeral,
            client_proof: &proof.client_proof,
            srp_session: auth_info_response.srp_session.as_ref(),
        }
        .execute_async::<T>(client, &DefaultRequestFactory {})
        .await?;

        validate_server_proof(&proof, &auth_response)
    }

    pub async fn refresh_async<T: http::ClientAsync>(
        client: &T,
        user_uid: &UserUid,
        token: &str,
    ) -> http::Result<Self> {
        let refresh_response = AuthRefreshRequest::new(user_uid, token)
            .execute_async(client, &DefaultRequestFactory {})
            .await?;
        let user = UserAuth::from_auth_refresh_response(&refresh_response);
        Ok(Session::new(user))
    }

    pub fn refresh<T: http::ClientSync>(
        &self,
        client: &T,
        user_uid: &UserUid,
        token: &str,
    ) -> http::Result<Self> {
        let refresh_response = AuthRefreshRequest::new(user_uid, token)
            .execute_sync(client, &DefaultRequestFactory {})?;
        let user = UserAuth::from_auth_refresh_response(&refresh_response);
        Ok(Session::new(user))
    }

    pub fn get_user<T: http::ClientSync>(&self, client: &T) -> Result<User, http::Error> {
        let user = UserInfoRequest {}.execute_sync::<T>(client, self)?;
        Ok(user.user)
    }

    pub async fn get_user_async<T: http::ClientAsync>(
        &self,
        client: &T,
    ) -> Result<User, http::Error> {
        let user = UserInfoRequest {}.execute_async::<T>(client, self).await?;
        Ok(user.user)
    }

    pub fn logout<T: http::ClientSync>(&self, client: &T) -> Result<(), http::Error> {
        LogoutRequest {}.execute_sync::<T>(client, self)
    }

    pub async fn logout_async<T: http::ClientAsync>(&self, client: &T) -> Result<(), http::Error> {
        LogoutRequest {}.execute_async::<T>(client, self).await
    }

    pub fn get_latest_event<T: http::ClientSync>(&self, client: &T) -> http::Result<EventId> {
        let r = GetLatestEventRequest.execute_sync(client, self)?;
        Ok(r.event_id)
    }

    pub async fn get_latest_event_async<T: http::ClientAsync>(
        &self,
        client: &T,
    ) -> http::Result<EventId> {
        let r = GetLatestEventRequest.execute_async(client, self).await?;
        Ok(r.event_id)
    }

    pub fn get_event<T: http::ClientSync>(&self, client: &T, id: &EventId) -> http::Result<Event> {
        GetEventRequest::new(id).execute_sync(client, self)
    }

    pub async fn get_event_async<T: http::ClientAsync>(
        &self,
        client: &T,
        id: &EventId,
    ) -> http::Result<Event> {
        GetEventRequest::new(id).execute_async(client, self).await
    }
}

impl RequestFactory for Session {
    fn new_request(&self, method: http::Method, url: &str) -> http::Request {
        self.new_request(method, url)
    }
}

fn generate_session_proof(
    username: &str,
    password: &str,
    auth_info_response: &AuthInfoResponse,
) -> Result<SRPAuth, LoginError> {
    SRPAuth::generate(
        username,
        password,
        auth_info_response.version,
        &auth_info_response.salt,
        &auth_info_response.modulus,
        &auth_info_response.server_ephemeral,
    )
    .map_err(LoginError::ServerProof)
}

fn validate_server_proof(
    proof: &SRPAuth,
    auth_response: &AuthResponse,
) -> Result<SessionType, LoginError> {
    if proof.expected_server_proof != auth_response.server_proof {
        return Err(LoginError::ServerProof(
            "Server Proof does not match".to_string(),
        ));
    }

    let user = UserAuth::from_auth_response(auth_response);

    let session = Session::new(user);

    match auth_response.tfa.enabled {
        TFAStatus::None => Ok(SessionType::Authenticated(session)),
        TFAStatus::Totp => Ok(SessionType::AwaitingTotp(TotpSession(session))),
        TFAStatus::FIDO2 => Err(LoginError::Unsupported2FA(TwoFactorAuth::FIDO2)),
    }
}
