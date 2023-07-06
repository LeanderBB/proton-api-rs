use crate::clientv2::TotpSession;
use crate::domain::{
    EventId, HumanVerification, HumanVerificationLoginData, SecretString, TwoFactorAuth, User,
    UserUid,
};
use crate::http;
use crate::http::{
    ClientAsync, ClientRequest, ClientRequestBuilder, ClientSync, FromResponse, Request,
    RequestDesc, Sequence, StateProducerSequence, X_PM_UID_HEADER,
};
use crate::requests::{
    AuthInfoRequest, AuthInfoResponse, AuthRefreshRequest, AuthRequest, AuthResponse,
    GetEventRequest, GetLatestEventRequest, LogoutRequest, TFAStatus, TOTPRequest, UserAuth,
    UserInfoRequest,
};
use go_srp::SRPAuth;
use secrecy::{ExposeSecret, Secret};
#[cfg(not(feature = "async-traits"))]
use std::future::Future;
#[cfg(not(feature = "async-traits"))]
use std::pin::Pin;
use std::sync::Arc;

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
    pub(super) user_auth: Arc<parking_lot::RwLock<UserAuth>>,
}

impl Session {
    fn new(user: UserAuth) -> Self {
        Self {
            user_auth: Arc::new(parking_lot::RwLock::new(user)),
        }
    }

    pub fn login<'a>(
        username: &'a str,
        password: &'a SecretString,
        human_verification: Option<HumanVerificationLoginData>,
    ) -> impl Sequence<'a, Output = SessionType, Error = LoginError> + 'a {
        let state = State {
            username,
            password,
            hv: human_verification,
        };

        StateProducerSequence::new(state, login_sequence_1)
    }

    pub fn submit_totp(&self, code: &str) -> impl Sequence<Output = (), Error = http::Error> {
        self.wrap_request(TOTPRequest::new(code).to_request())
    }

    pub fn refresh<'a>(
        user_uid: &'a UserUid,
        token: &'a str,
    ) -> impl Sequence<'a, Output = Self, Error = http::Error> + 'a {
        AuthRefreshRequest::new(user_uid, token)
            .to_request()
            .map(|r| {
                let user = UserAuth::from_auth_refresh_response(r);
                Ok(Session::new(user))
            })
    }

    pub fn get_user(&self) -> impl Sequence<Output = User> {
        self.wrap_request(UserInfoRequest {}.to_request())
            .map(|r| -> Result<User, http::Error> { Ok(r.user) })
    }

    pub fn logout(&self) -> impl Sequence<Output = ()> {
        self.wrap_request(LogoutRequest {}.to_request())
    }

    pub fn get_latest_event(&self) -> impl Request {
        self.wrap_request(GetLatestEventRequest {}.to_request())
    }

    pub fn get_event(&self, id: &EventId) -> impl Request {
        self.wrap_request(GetEventRequest::new(id).to_request())
    }

    pub fn get_refresh_data(&self) -> SessionRefreshData {
        let reader = self.user_auth.read();
        SessionRefreshData {
            user_uid: reader.uid.clone(),
            token: reader.refresh_token.clone(),
        }
    }

    #[inline(always)]
    fn wrap_request<R: Request>(&self, r: R) -> SessionRequest<R> {
        SessionRequest(r, self.user_auth.clone())
    }
}

fn validate_server_proof(
    proof: &SRPAuth,
    auth_response: AuthResponse,
) -> Result<SessionType, LoginError> {
    if proof.expected_server_proof != auth_response.server_proof {
        return Err(LoginError::ServerProof(
            "Server Proof does not match".to_string(),
        ));
    }

    let tfa_enabled = auth_response.tfa.enabled;
    let user = UserAuth::from_auth_response(auth_response);

    let session = Session::new(user);

    match tfa_enabled {
        TFAStatus::None => Ok(SessionType::Authenticated(session)),
        TFAStatus::Totp => Ok(SessionType::AwaitingTotp(TotpSession(session))),
        TFAStatus::FIDO2 => Err(LoginError::Unsupported2FA(TwoFactorAuth::FIDO2)),
        TFAStatus::TotpOrFIDO2 => Ok(SessionType::AwaitingTotp(TotpSession(session))),
    }
}

fn map_human_verification_err(e: LoginError) -> LoginError {
    if let LoginError::Request(http::Error::API(e)) = &e {
        if let Ok(hv) = e.try_get_human_verification_details() {
            return LoginError::HumanVerificationRequired(hv);
        }
    }

    e
}

pub struct SessionRequest<R: Request>(R, Arc<parking_lot::RwLock<UserAuth>>);

impl<R: Request> SessionRequest<R> {
    fn refresh_auth(&self) -> impl Sequence<'_, Output = (), Error = http::Error> + '_ {
        let reader = self.1.read();
        AuthRefreshRequest::new(
            reader.uid.expose_secret(),
            reader.refresh_token.expose_secret(),
        )
        .to_request()
        .map(|resp| {
            let mut writer = self.1.write();
            *writer = UserAuth::from_auth_refresh_response(resp);
            Ok(())
        })
    }

    async fn exec_async_impl<'a, C: ClientAsync, F: FromResponse>(
        &'a self,
        client: &'a C,
    ) -> Result<F::Output, http::Error> {
        let v = self.build(client);
        match client.execute_async::<F>(v).await {
            Ok(r) => Ok(r),
            Err(original_error) => {
                if let http::Error::API(api_err) = &original_error {
                    if api_err.http_code == 401 {
                        log::debug!("Account session expired, attempting refresh");
                        // Session expired/not authorized, try auth refresh.
                        if let Err(e) = self.refresh_auth().do_async(client).await {
                            log::error!("Failed to refresh account {e}");
                            return Err(original_error);
                        }

                        // Execute request again
                        return client.execute_async::<F>(self.build(client)).await;
                    }
                }
                Err(original_error)
            }
        }
    }
}

impl<R: Request> Request for SessionRequest<R> {
    type Response = R::Response;

    fn build<C: ClientRequestBuilder>(&self, builder: &C) -> C::Request {
        let r = self.0.build(builder);
        let borrow = self.1.read();
        r.header(X_PM_UID_HEADER, borrow.uid.expose_secret().as_str())
            .bearer_token(borrow.access_token.expose_secret())
    }

    fn exec_sync<T: ClientSync>(
        &self,
        client: &T,
    ) -> Result<<Self::Response as FromResponse>::Output, http::Error> {
        match client.execute::<Self::Response>(self.build(client)) {
            Ok(r) => Ok(r),
            Err(original_error) => {
                if let http::Error::API(api_err) = &original_error {
                    if api_err.http_code == 401 {
                        log::debug!("Account session expired, attempting refresh");
                        // Session expired/not authorized, try auth refresh.
                        if let Err(e) = self.refresh_auth().do_sync(client) {
                            log::error!("Failed to refresh account {e}");
                            return Err(original_error);
                        }

                        // Execute request again
                        return client.execute::<Self::Response>(self.build(client));
                    }
                }
                Err(original_error)
            }
        }
    }

    #[cfg(not(feature = "async-traits"))]
    fn exec_async<'a, T: ClientAsync>(
        &'a self,
        client: &'a T,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<<Self::Response as FromResponse>::Output, http::Error>> + 'a,
        >,
    > {
        Box::pin(async move { self.exec_async_impl::<T, R::Response>(client).await })
    }

    #[cfg(feature = "async-traits")]
    async fn exec_async<'a, T: ClientAsync>(
        &'a self,
        client: &'a T,
    ) -> Result<<Self::Response as FromResponse>::Output, http::Error> {
        self.exec_async_impl::<T, R::Response>(client).await
    }
}

struct State<'a> {
    username: &'a str,
    password: &'a SecretString,
    hv: Option<HumanVerificationLoginData>,
}

struct LoginState<'a> {
    username: &'a str,
    proof: SRPAuth,
    session: String,
    hv: Option<HumanVerificationLoginData>,
}

fn generate_login_state(
    state: State,
    auth_info_response: AuthInfoResponse,
) -> Result<LoginState, LoginError> {
    let proof = SRPAuth::generate(
        state.username,
        state.password.expose_secret(),
        auth_info_response.version,
        &auth_info_response.salt,
        &auth_info_response.modulus,
        &auth_info_response.server_ephemeral,
    )
    .map_err(LoginError::ServerProof)?;

    Ok(LoginState {
        username: state.username,
        proof,
        session: auth_info_response.srp_session,
        hv: state.hv,
    })
}

fn login_sequence_2(
    login_state: LoginState,
) -> impl Sequence<'_, Output = SessionType, Error = LoginError> + '_ {
    AuthRequest {
        username: login_state.username,
        client_ephemeral: &login_state.proof.client_ephemeral,
        client_proof: &login_state.proof.client_proof,
        srp_session: &login_state.session,
        human_verification: &login_state.hv,
    }
    .to_request()
    .map(move |auth_response| {
        validate_server_proof(&login_state.proof, auth_response).map_err(map_human_verification_err)
    })
}

fn login_sequence_1(st: State) -> impl Sequence<'_, Output = SessionType, Error = LoginError> + '_ {
    AuthInfoRequest {
        username: st.username,
    }
    .to_request()
    .map(move |auth_info_response| generate_login_state(st, auth_info_response))
    .state(login_sequence_2)
}
