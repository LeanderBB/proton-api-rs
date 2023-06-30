use crate::domain::{HumanVerificationLoginData, SecretString, UserUid};
use crate::http;
use crate::http::{
    RequestData, RequestFactory, X_PM_HUMAN_VERIFICATION_TOKEN, X_PM_HUMAN_VERIFICATION_TOKEN_TYPE,
};
use secrecy::Secret;
use serde::{Deserialize, Serialize};
use serde_repr::Deserialize_repr;
use std::borrow::Cow;

#[doc(hidden)]
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthInfoRequest<'a> {
    pub username: &'a str,
}

impl<'a> http::Request for AuthInfoRequest<'a> {
    type Output = AuthInfoResponse<'a>;
    type Response = http::JsonResponse<Self::Output>;

    fn build_request(&self, factory: &dyn RequestFactory) -> RequestData {
        factory
            .new_request(http::Method::Post, "auth/v4/info")
            .json(self)
    }
}

#[doc(hidden)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct AuthInfoResponse<'a> {
    pub version: i64,
    pub modulus: Cow<'a, str>,
    pub server_ephemeral: Cow<'a, str>,
    pub salt: Cow<'a, str>,
    #[serde(rename = "SRPSession")]
    pub srp_session: Cow<'a, str>,
}

#[doc(hidden)]
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthRequest<'a> {
    pub username: &'a str,
    pub client_ephemeral: &'a str,
    pub client_proof: &'a str,
    #[serde(rename = "SRPSession")]
    pub srp_session: &'a str,
    #[serde(skip)]
    pub human_verification: Option<HumanVerificationLoginData>,
}

impl<'a> http::Request for AuthRequest<'a> {
    type Output = AuthResponse<'a>;
    type Response = http::JsonResponse<Self::Output>;

    fn build_request(&self, factory: &dyn RequestFactory) -> RequestData {
        let mut request = factory
            .new_request(http::Method::Post, "auth/v4")
            .json(self);

        if let Some(hv) = &self.human_verification {
            // repeat submission with x-pm-human-verification-token and x-pm-human-verification-token-type
            request = request
                .header(X_PM_HUMAN_VERIFICATION_TOKEN, &hv.token)
                .header(X_PM_HUMAN_VERIFICATION_TOKEN_TYPE, hv.hv_type.as_str())
        }

        request
    }
}

#[doc(hidden)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct AuthResponse<'a> {
    #[serde(rename = "UserID")]
    pub user_id: Cow<'a, str>,
    #[serde(rename = "UID")]
    pub uid: Cow<'a, str>,
    pub token_type: Option<Cow<'a, str>>,
    pub access_token: Cow<'a, str>,
    pub refresh_token: Cow<'a, str>,
    pub server_proof: Cow<'a, str>,
    pub scope: Cow<'a, str>,
    #[serde(rename = "2FA")]
    pub tfa: TFAInfo<'a>,
    pub password_mode: PasswordMode,
}

#[doc(hidden)]
#[derive(Deserialize_repr, Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum PasswordMode {
    One = 1,
    Two = 2,
}

#[doc(hidden)]
#[derive(Deserialize_repr, Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum TFAStatus {
    None = 0,
    Totp = 1,
    FIDO2 = 2,
    TotpOrFIDO2 = 3,
}

#[doc(hidden)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct TFAInfo<'a> {
    pub enabled: TFAStatus,
    #[serde(rename = "FIDO2")]
    pub fido2_info: FIDO2Info<'a>,
}

#[doc(hidden)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct FIDOKey<'a> {
    pub attestation_format: Cow<'a, str>,
    #[serde(rename = "CredentialID")]
    pub credential_id: Vec<i32>,
    pub name: Cow<'a, str>,
}

#[doc(hidden)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct FIDO2Info<'a> {
    pub authentication_options: serde_json::Value,
    pub registered_keys: Option<Vec<FIDOKey<'a>>>,
}

#[doc(hidden)]
#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct TFAAuth<'a> {
    pub two_factor_code: &'a str,
    #[serde(rename = "FIDO2")]
    pub fido2: FIDO2Auth<'a>,
}

#[doc(hidden)]
#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct FIDO2Auth<'a> {
    pub authentication_options: serde_json::Value,
    pub client_data: &'a str,
    pub authentication_data: &'a str,
    pub signature: &'a str,
    #[serde(rename = "CredentialID")]
    pub credential_id: &'a [i32],
}

impl<'a> FIDO2Auth<'a> {
    pub fn empty() -> Self {
        FIDO2Auth {
            authentication_options: serde_json::Value::Null,
            client_data: "",
            authentication_data: "",
            signature: "",
            credential_id: &[],
        }
    }
}

pub struct TOTPRequest<'a> {
    code: &'a str,
}

impl<'a> TOTPRequest<'a> {
    pub fn new(code: &'a str) -> Self {
        Self { code }
    }
}

impl<'a> http::Request for TOTPRequest<'a> {
    type Output = ();
    type Response = http::NoResponse;

    fn build_request(&self, factory: &dyn RequestFactory) -> RequestData {
        factory
            .new_request(http::Method::Post, "auth/v4/2fa")
            .json(TFAAuth {
                two_factor_code: self.code,
                fido2: FIDO2Auth::empty(),
            })
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct UserAuth {
    pub uid: Secret<UserUid>,
    pub access_token: SecretString,
    pub refresh_token: SecretString,
}

impl UserAuth {
    pub fn from_auth_response(auth: &AuthResponse) -> Self {
        Self {
            uid: Secret::new(UserUid(auth.uid.to_string())),
            access_token: SecretString::new(auth.access_token.to_string()),
            refresh_token: SecretString::new(auth.refresh_token.to_string()),
        }
    }

    pub fn from_auth_refresh_response(auth: &AuthRefreshResponse) -> Self {
        Self {
            uid: Secret::new(UserUid(auth.uid.to_string())),
            access_token: SecretString::new(auth.access_token.to_string()),
            refresh_token: SecretString::new(auth.refresh_token.to_string()),
        }
    }
}

#[doc(hidden)]
#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct AuthRefresh<'a> {
    #[serde(rename = "UID")]
    pub uid: &'a str,
    pub refresh_token: &'a str,
    pub grant_type: &'a str,
    pub response_type: &'a str,
    #[serde(rename = "RedirectURI")]
    pub redirect_uri: &'a str,
}

#[doc(hidden)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct AuthRefreshResponse<'a> {
    #[serde(rename = "UID")]
    pub uid: Cow<'a, str>,
    pub token_type: Cow<'a, str>,
    pub access_token: Cow<'a, str>,
    pub refresh_token: Cow<'a, str>,
    pub scope: Cow<'a, str>,
}

pub struct AuthRefreshRequest<'a> {
    uid: &'a UserUid,
    token: &'a str,
}

impl<'a> AuthRefreshRequest<'a> {
    pub fn new(uid: &'a UserUid, token: &'a str) -> Self {
        Self { uid, token }
    }
}

impl<'a> http::Request for AuthRefreshRequest<'a> {
    type Output = AuthRefreshResponse<'a>;
    type Response = http::JsonResponse<Self::Output>;

    fn build_request(&self, factory: &dyn RequestFactory) -> RequestData {
        factory
            .new_request(http::Method::Post, "auth/v4/refresh")
            .json(AuthRefresh {
                uid: &self.uid.0,
                refresh_token: self.token,
                grant_type: "refresh_token",
                response_type: "token",
                redirect_uri: "https://protonmail.ch/",
            })
    }
}

pub struct LogoutRequest {}

impl http::Request for LogoutRequest {
    type Output = ();
    type Response = http::NoResponse;

    fn build_request(&self, factory: &dyn RequestFactory) -> RequestData {
        factory.new_request(http::Method::Delete, "auth/v4")
    }
}

pub struct CaptchaRequest<'a> {
    token: &'a str,
    force_web: bool,
}

impl<'a> CaptchaRequest<'a> {
    pub fn new(token: &'a str, force_web: bool) -> Self {
        Self { token, force_web }
    }
}

impl<'a> http::Request for CaptchaRequest<'a> {
    type Output = String;
    type Response = http::StringResponse;

    fn build_request(&self, factory: &dyn RequestFactory) -> RequestData {
        let url = if self.force_web {
            format!("core/v4/captcha?ForceWebMessaging=1&Token={}", self.token)
        } else {
            format!("core/v4/captcha?Token={}", self.token)
        };
        factory.new_request(http::Method::Get, &url)
    }
}
