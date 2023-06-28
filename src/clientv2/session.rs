use crate::clientv2::request_repeater::RequestRepeater;
use crate::clientv2::TotpSession;
use crate::domain::{
    Event, EventId, HumanVerification, HumanVerificationLoginData, TwoFactorAuth, User, UserUid,
};
use crate::http::{DefaultRequestFactory, Request};
use crate::requests::{
    AuthInfoRequest, AuthInfoResponse, AuthRefreshRequest, AuthRequest, AuthResponse,
    GetEventRequest, GetLatestEventRequest, LogoutRequest, TFAStatus, UserAuth, UserInfoRequest,
};
use crate::{http, OnAuthRefreshed};
use go_srp::SRPAuth;
use secrecy::Secret;

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
    #[error("Human Verification Required'")]
    HumanVerificationRequired(HumanVerification),
    #[error("Failed to calculate SRP Proof: {0}")]
    SRPProof(String),
}

/// Data which can be used to save a session and restore it later.
pub struct SessionRefreshData {
    pub user_uid: Secret<UserUid>,
    pub token: Secret<String>,
}

#[derive(Debug)]
pub enum SessionType {
    Authenticated(Session),
    AwaitingTotp(TotpSession),
}

/// Authenticated Session from which one can access data/functionality restricted to authenticated
/// users.
#[derive(Debug)]
pub struct Session {
    pub(super) repeater: RequestRepeater,
}

impl Session {
    fn new(user: UserAuth, on_auth_refreshed_cb: Option<Box<dyn OnAuthRefreshed>>) -> Self {
        Self {
            repeater: RequestRepeater::new(user, on_auth_refreshed_cb),
        }
    }

    pub fn login<T: http::ClientSync>(
        client: &T,
        username: &str,
        password: &str,
        human_verification: Option<HumanVerificationLoginData>,
        on_auth_refreshed: Option<Box<dyn OnAuthRefreshed>>,
    ) -> Result<SessionType, LoginError> {
        let auth_info_response =
            AuthInfoRequest { username }.execute_sync::<T>(client, &DefaultRequestFactory {})?;

        let proof = generate_session_proof(username, password, &auth_info_response)?;

        let auth_response = AuthRequest {
            username,
            client_ephemeral: &proof.client_ephemeral,
            client_proof: &proof.client_proof,
            srp_session: auth_info_response.srp_session.as_ref(),
            human_verification,
        }
        .execute_sync::<T>(client, &DefaultRequestFactory {})
        .map_err(map_human_verification_err)?;

        validate_server_proof(&proof, &auth_response, on_auth_refreshed)
    }

    pub async fn login_async<T: http::ClientAsync>(
        client: &T,
        username: &str,
        password: &str,
        human_verification: Option<HumanVerificationLoginData>,
        on_auth_refreshed: Option<Box<dyn OnAuthRefreshed>>,
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
            human_verification,
        }
        .execute_async::<T>(client, &DefaultRequestFactory {})
        .await
        .map_err(map_human_verification_err)?;

        validate_server_proof(&proof, &auth_response, on_auth_refreshed)
    }

    pub async fn refresh_async<T: http::ClientAsync>(
        client: &T,
        user_uid: &UserUid,
        token: &str,
        on_auth_refreshed: Option<Box<dyn OnAuthRefreshed>>,
    ) -> http::Result<Self> {
        let refresh_response = AuthRefreshRequest::new(user_uid, token)
            .execute_async(client, &DefaultRequestFactory {})
            .await?;
        let user = UserAuth::from_auth_refresh_response(&refresh_response);
        Ok(Session::new(user, on_auth_refreshed))
    }

    pub fn refresh<T: http::ClientSync>(
        client: &T,
        user_uid: &UserUid,
        token: &str,
        on_auth_refreshed: Option<Box<dyn OnAuthRefreshed>>,
    ) -> http::Result<Self> {
        let refresh_response = AuthRefreshRequest::new(user_uid, token)
            .execute_sync(client, &DefaultRequestFactory {})?;
        let user = UserAuth::from_auth_refresh_response(&refresh_response);
        Ok(Session::new(user, on_auth_refreshed))
    }

    pub fn get_user<T: http::ClientSync>(&self, client: &T) -> Result<User, http::Error> {
        let user = self.repeater.execute(client, UserInfoRequest {})?;
        Ok(user.user)
    }

    pub async fn get_user_async<T: http::ClientAsync>(
        &self,
        client: &T,
    ) -> Result<User, http::Error> {
        let user = self
            .repeater
            .execute_async(client, UserInfoRequest {})
            .await?;
        Ok(user.user)
    }

    pub fn logout<T: http::ClientSync>(&self, client: &T) -> Result<(), http::Error> {
        LogoutRequest {}.execute_sync::<T>(client, &self.repeater)
    }

    pub async fn logout_async<T: http::ClientAsync>(&self, client: &T) -> Result<(), http::Error> {
        LogoutRequest {}
            .execute_async::<T>(client, &self.repeater)
            .await
    }

    pub fn get_latest_event<T: http::ClientSync>(&self, client: &T) -> http::Result<EventId> {
        let r = self.repeater.execute(client, GetLatestEventRequest {})?;
        Ok(r.event_id)
    }

    pub async fn get_latest_event_async<T: http::ClientAsync>(
        &self,
        client: &T,
    ) -> http::Result<EventId> {
        let r = self
            .repeater
            .execute_async(client, GetLatestEventRequest {})
            .await?;
        Ok(r.event_id)
    }

    pub fn get_event<T: http::ClientSync>(&self, client: &T, id: &EventId) -> http::Result<Event> {
        self.repeater.execute(client, GetEventRequest::new(id))
    }

    pub async fn get_event_async<T: http::ClientAsync>(
        &self,
        client: &T,
        id: &EventId,
    ) -> http::Result<Event> {
        self.repeater
            .execute_async(client, GetEventRequest::new(id))
            .await
    }

    pub fn get_refresh_data(&self) -> SessionRefreshData {
        self.repeater.get_refresh_data()
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
    on_auth_refreshed: Option<Box<dyn OnAuthRefreshed>>,
) -> Result<SessionType, LoginError> {
    if proof.expected_server_proof != auth_response.server_proof {
        return Err(LoginError::ServerProof(
            "Server Proof does not match".to_string(),
        ));
    }

    let user = UserAuth::from_auth_response(auth_response);

    let session = Session::new(user, on_auth_refreshed);

    match auth_response.tfa.enabled {
        TFAStatus::None => Ok(SessionType::Authenticated(session)),
        TFAStatus::Totp => Ok(SessionType::AwaitingTotp(TotpSession(session))),
        TFAStatus::FIDO2 => Err(LoginError::Unsupported2FA(TwoFactorAuth::FIDO2)),
        TFAStatus::TotpOrFIDO2 => Ok(SessionType::AwaitingTotp(TotpSession(session))),
    }
}

fn map_human_verification_err(e: http::Error) -> LoginError {
    if let http::Error::API(e) = &e {
        if let Ok(hv) = e.try_get_human_verification_details() {
            return LoginError::HumanVerificationRequired(hv);
        }
    }

    LoginError::from(e)
}
