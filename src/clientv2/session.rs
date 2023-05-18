use crate::clientv2::TotpSession;
use crate::domain::{Event, EventId, TwoFactorAuth, User, UserUid};
use crate::http::{DefaultRequestFactory, Request, RequestFactory};
use crate::requests::{
    AuthInfoRequest, AuthInfoResponse, AuthRefreshRequest, AuthRequest, AuthResponse,
    GetEventRequest, GetLatestEventRequest, LogoutRequest, TFAStatus, UserAuth, UserInfoRequest,
};
use crate::{http, AutoAuthRefreshRequestPolicy};
use go_srp::SRPAuth;
use secrecy::Secret;
use std::future::Future;
use std::pin::Pin;

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

/// Trait to access the underlying user authentication data in a session policy.
pub trait UserAuthFromSessionRequestPolicy {
    fn user_auth(&self) -> &UserAuth;
    fn user_auth_mut(&mut self) -> &mut UserAuth;
}

/// Data which can be used to save a session and restore it later.
pub struct SessionRefreshData {
    pub user_uid: Secret<UserUid>,
    pub token: Secret<String>,
}

/// Session Request Policy can be used to add custom behavior to a session request, such as
/// retrying on network errors, respecting 429 codes, etc...
pub trait SessionRequestPolicy: From<UserAuth> + RequestFactory {
    fn execute<C: http::ClientSync, R: Request>(
        &self,
        client: &C,
        request: R,
    ) -> http::Result<R::Output>;

    fn execute_async<'a, C: http::ClientAsync, R: Request + 'a>(
        &'a self,
        client: &'a C,
        request: R,
    ) -> Pin<Box<dyn Future<Output = http::Result<R::Output>> + 'a>>;

    fn get_refresh_data(&self) -> SessionRefreshData;
}

#[derive(Debug)]
pub enum SessionType<P: SessionRequestPolicy> {
    Authenticated(Session<P>),
    AwaitingTotp(TotpSession<P>),
}

/// Authenticated Session from which one can access data/functionality restricted to authenticated
/// users.
#[derive(Debug)]
pub struct Session<P: SessionRequestPolicy> {
    pub(super) policy: P,
}

impl<P: SessionRequestPolicy> Session<P> {
    fn new(user: UserAuth) -> Self {
        Self {
            policy: P::from(user),
        }
    }

    pub fn login<T: http::ClientSync>(
        client: &T,
        username: &str,
        password: &str,
    ) -> Result<SessionType<P>, LoginError> {
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
    ) -> Result<SessionType<P>, LoginError> {
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
        let user = self.policy.execute(client, UserInfoRequest {})?;
        Ok(user.user)
    }

    pub async fn get_user_async<T: http::ClientAsync>(
        &self,
        client: &T,
    ) -> Result<User, http::Error> {
        let user = self
            .policy
            .execute_async(client, UserInfoRequest {})
            .await?;
        Ok(user.user)
    }

    pub fn logout<T: http::ClientSync>(&self, client: &T) -> Result<(), http::Error> {
        LogoutRequest {}.execute_sync::<T>(client, &self.policy)
    }

    pub async fn logout_async<T: http::ClientAsync>(&self, client: &T) -> Result<(), http::Error> {
        LogoutRequest {}
            .execute_async::<T>(client, &self.policy)
            .await
    }

    pub fn get_latest_event<T: http::ClientSync>(&self, client: &T) -> http::Result<EventId> {
        let r = self.policy.execute(client, GetLatestEventRequest {})?;
        Ok(r.event_id)
    }

    pub async fn get_latest_event_async<T: http::ClientAsync>(
        &self,
        client: &T,
    ) -> http::Result<EventId> {
        let r = self
            .policy
            .execute_async(client, GetLatestEventRequest {})
            .await?;
        Ok(r.event_id)
    }

    pub fn get_event<T: http::ClientSync>(&self, client: &T, id: &EventId) -> http::Result<Event> {
        self.policy.execute(client, GetEventRequest::new(id))
    }

    pub async fn get_event_async<T: http::ClientAsync>(
        &self,
        client: &T,
        id: &EventId,
    ) -> http::Result<Event> {
        self.policy
            .execute_async(client, GetEventRequest::new(id))
            .await
    }

    pub fn get_refresh_data(&self) -> SessionRefreshData {
        self.policy.get_refresh_data()
    }
}

impl<T: SessionRequestPolicy + UserAuthFromSessionRequestPolicy>
    Session<AutoAuthRefreshRequestPolicy<T>>
{
    pub fn was_auth_refreshed(&self) -> bool {
        self.policy.was_auth_refreshed()
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

fn validate_server_proof<P: SessionRequestPolicy>(
    proof: &SRPAuth,
    auth_response: &AuthResponse,
) -> Result<SessionType<P>, LoginError> {
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
