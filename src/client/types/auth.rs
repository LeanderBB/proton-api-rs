use crate::domain::{SecretString, UserUid};
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
}

#[doc(hidden)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct AuthResponse<'a> {
    #[serde(rename = "UserID")]
    pub user_id: Cow<'a, str>,
    #[serde(rename = "UID")]
    pub uid: Cow<'a, str>,
    pub token_type: Cow<'a, str>,
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
    pub registered_keys: Vec<FIDOKey<'a>>,
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

#[doc(hidden)]
#[derive(Debug)]
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
