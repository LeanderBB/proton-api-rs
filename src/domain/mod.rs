//! Domain Types.

mod event;
mod user;

pub use event::*;
pub use user::*;

use std::fmt::{Display, Formatter};

pub type SecretString = secrecy::SecretString;
pub use secrecy::ExposeSecret;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
/// Types of Two Factor Authentication.
pub enum TwoFactorAuth {
    None,
    TOTP,
    FIDO2,
}

impl Display for TwoFactorAuth {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TwoFactorAuth::None => "None".fmt(f),
            TwoFactorAuth::TOTP => "TOTP".fmt(f),
            TwoFactorAuth::FIDO2 => "FIDO2".fmt(f),
        }
    }
}
