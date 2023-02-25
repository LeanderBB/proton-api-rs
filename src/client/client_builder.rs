use crate::client::types::{
    AuthInfoRequest, AuthInfoResponse, AuthRefresh, AuthRefreshResponse, AuthRequest, AuthResponse,
    FIDO2Auth, TFAAuth, TFAStatus, UserAuth,
};
use crate::client::{HttpClientBuilder, X_PM_UID_HEADER};
use crate::domain::UserUid;
use crate::{impl_error_conversion, Client, RequestError};
use go_srp::SRPAuth;
use std::time::Duration;
use thiserror::Error;

/// After constructing a client
pub enum ClientLoginState {
    /// Client is fully authenticated and ready to be used.
    Authenticated(Client),
    /// User account needs 2FA TOTP verification before proceeding.
    AwaitingTotp(TOTPClient),
}

/// Represents the errors that may occur when building an new `Client`.
#[derive(Debug, Error)]
pub enum ClientBuilderError {
    #[error("Server SRP proof verification failed: {0}")]
    ServerProof(String),
    #[error("{0}")]
    Request(
        #[from]
        #[source]
        RequestError,
    ),
    #[error("Account 2FA method ({0})is not supported")]
    Unsupported2FA(crate::domain::TwoFactorAuth),
    #[error("Failed to calculate SRP Proof: {0}")]
    SRPProof(String),
}

impl_error_conversion!(ClientBuilderError);

/// Configure client details. The type can be re-used for multiple clients if you clone
/// before consuming the type.
#[derive(Clone)]
pub struct ClientBuilder(HttpClientBuilder);

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self(Default::default())
    }

    /// Set the app version for this client e.g.: my-client@1.4.0+beta.
    /// Note: The default app version is not guaranteed to be accepted by the proton servers.
    pub fn app_version(mut self, version: &str) -> Self {
        self.0 = self.0.app_version(version);
        self
    }

    /// Set the user agent to be submitted with every request.
    pub fn user_agent(mut self, agent: &str) -> Self {
        self.0 = self.0.user_agent(agent);
        self
    }

    /// Set server's base url. By default the proton API server url is used.
    pub fn base_url(mut self, url: &str) -> Self {
        self.0 = self.0.base_url(url);
        self
    }

    /// Set the request timeout. By default the timeout is set to 5 seconds.
    pub fn request_timeout(mut self, duration: Duration) -> Self {
        self.0 = self.0.request_timeout(duration);
        self
    }

    /// Login into a proton account and start a new session.
    /// Note: At the moment we only support TOTP 2FA, `LoginError::Unsupported2FA` will returned
    /// if anther 2FA method is enabled.
    pub async fn login(
        self,
        username: &str,
        password: &str,
    ) -> Result<ClientLoginState, ClientBuilderError> {
        let http_client = self.0.build()?;

        let info_response = http_client
            .post("/auth/v4/info")
            .with_body(&AuthInfoRequest { username })
            .execute()
            .await?;

        let info_body_bytes = info_response.into_bytes().await?;
        let info = info_body_bytes.as_json::<AuthInfoResponse>()?;

        let proof = match SRPAuth::generate(
            username,
            password,
            info.version,
            &info.salt,
            &info.modulus,
            &info.server_ephemeral,
        ) {
            Ok(p) => p,
            Err(e) => return Err(ClientBuilderError::SRPProof(e)),
        };

        let auth_resp = http_client
            .post("/auth/v4")
            .with_body(&AuthRequest {
                username,
                client_ephemeral: &proof.client_ephemeral,
                client_proof: &proof.client_proof,
                srp_session: info.srp_session.as_ref(),
            })
            .execute()
            .await?;

        let auth_bytes = auth_resp.into_bytes().await?;
        let auth = auth_bytes.as_json::<AuthResponse>()?;

        if proof.expected_server_proof != auth.server_proof {
            return Err(ClientBuilderError::ServerProof(
                "Server Proof does not match".to_string(),
            ));
        }

        let user = UserAuth::from_auth_response(&auth);

        let authenticated_client = Client { http_client, user };

        match auth.tfa.enabled {
            TFAStatus::None => Ok(ClientLoginState::Authenticated(authenticated_client)),
            TFAStatus::Totp => Ok(ClientLoginState::AwaitingTotp(TOTPClient(
                authenticated_client,
            ))),
            TFAStatus::FIDO2 => Err(ClientBuilderError::Unsupported2FA(
                crate::domain::TwoFactorAuth::FIDO2,
            )),
        }
    }

    /// Login into an account using a refresh token. The `user_uid` and `refresh_token` can
    /// be obtained after a successful login.
    pub async fn with_token(
        self,
        user_uid: &UserUid,
        refresh_token: &str,
    ) -> Result<Client, RequestError> {
        let http_client = self.0.build()?;

        let response = http_client
            .post("/auth/v4/refresh")
            .header(X_PM_UID_HEADER, &user_uid.0)
            .with_body(&AuthRefresh {
                uid: &user_uid.0,
                refresh_token,
                grant_type: "refresh_token",
                response_type: "token",
                redirect_uri: "https://protonmail.ch/",
            })
            .execute()
            .await?;

        let auth = response.json::<AuthRefreshResponse>().await?;
        let user = UserAuth::from_auth_refresh_response(&auth);

        Ok(Client { http_client, user })
    }
}

#[derive(Debug)]
/// If an account requires TOTP 2FA this type is returned from `ClientBuilder::login`.
pub struct TOTPClient(Client);

impl TOTPClient {
    /// Submit the TOTP request to advance the client to the fully authenticated state.
    pub async fn submit_totp(self, code: &str) -> Result<Client, (Self, RequestError)> {
        if let Err(e) = self
            .0
            .post("/auth/v4/2fa")
            .with_body(&TFAAuth {
                two_factor_code: code,
                fido2: FIDO2Auth::empty(),
            })
            .execute()
            .await
        {
            return Err((self, e));
        }

        Ok(self.0)
    }
}
