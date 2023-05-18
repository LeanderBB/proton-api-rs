// Enable clippy if our Cargo.toml file asked us to do so.
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]
// Enable as many useful Rust and Clippy warnings as we can stand.
#![warn(
    missing_copy_implementations,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    trivial_casts,
    unused_qualifications
)]
#![cfg_attr(feature = "clippy", warn(cast_possible_truncation))]
#![cfg_attr(feature = "clippy", warn(cast_possible_wrap))]
#![cfg_attr(feature = "clippy", warn(cast_precision_loss))]
#![cfg_attr(feature = "clippy", warn(cast_sign_loss))]
#![cfg_attr(feature = "clippy", warn(missing_docs_in_private_items))]
#![cfg_attr(feature = "clippy", warn(mut_mut))]
// Disallow `println!`. Use `debug!` for debug output
// (which is provided by the `log` crate).
#![cfg_attr(feature = "clippy", warn(print_stdout))]
// This allows us to use `unwrap` on `Option` values (because doing makes
// working with Regex matches much nicer) and when compiling in test mode
// (because using it in tests is idiomatic).
#![cfg_attr(all(not(test), feature = "clippy"), warn(result_unwrap_used))]
#![cfg_attr(feature = "clippy", warn(unseparated_literal_suffix))]
#![cfg_attr(feature = "clippy", warn(wrong_pub_self_convention))]

//! Unofficial Rust bindings for the REST API for Proton. It is all based on the information
//! available from the [go-proton-api](https://github.com/ProtonMail/go-proton-api) and the
//! [Proton Bridge](https://github.com/ProtonMail/proton-bridge) repositories.
//!
//! # Disclaimer
//! These are **UNOFFICIAL** bindings, use at your own risk. The author will not be held liable if
//! you experience data loss or your account gets blocked.
//!
//! # Getting Started
//!
//! Login into a new session async:
//! ```
//! use proton_api_rs::{http, DefaultSession, SessionType};
//! async fn example<T:http::ClientAsync>() {
//!     let client = http::ClientBuilder::new()
//!         .user_agent("MyUserAgent/0.0.0")
//!         .base_url("server_url")
//!         .app_version("MyApp@0.1.1")
//!         .build::<T>().unwrap();
//!
//!     let session = match DefaultSession::login_async(&client, "my_address@proton.me", "my_proton_password").await.unwrap(){
//!         // Session is authenticated, no 2FA verifications necessary.
//!         SessionType::Authenticated(c) => c,
//!         // Session needs 2FA TOTP auth.
//!         SessionType::AwaitingTotp(t) => {
//!             t.submit_totp_async(&client, "000000").await.unwrap()
//!         }
//!     };
//!
//!     // session is now authenticated and can access the rest of the API.
//!     // ...
//!
//!     session.logout_async(&client).await.unwrap();
//! }
//! ```
//!
//! Login into a new session sync:
//! ```
//! use proton_api_rs::{DefaultSession, http, Session, SessionType};
//! fn example<T:http::ClientSync>() {
//!     let client = http::ClientBuilder::new()
//!         .user_agent("MyUserAgent/0.0.0")
//!         .base_url("server_url")
//!         .app_version("MyApp@0.1.1")
//!         .build::<T>().unwrap();
//!
//!     let session = match DefaultSession::login(&client, "my_address@proton.me", "my_proton_password").unwrap(){
//!         // Session is authenticated, no 2FA verifications necessary.
//!         SessionType::Authenticated(c) => c,
//!         // Session needs 2FA TOTP auth.
//!         SessionType::AwaitingTotp(t) => {
//!             t.submit_totp(&client, "000000").unwrap()
//!         }
//!     };
//!
//!     // session is now authenticated and can access the rest of the API.
//!     // ...
//!
//!     session.logout(&client).unwrap();
//! }
//! ```
//!
//! Login using a previous sessions token.
//! ```
//! use proton_api_rs::{http, DefaultSession, SessionType};
//! use proton_api_rs::domain::UserUid;
//!
//! async fn example<T:http::ClientAsync>() {
//!     let user_uid = "user_uid".into();
//!     let user_refresh_token = "token";
//!     let client = http::ClientBuilder::new()
//!         .user_agent("MyUserAgent/0.0.0")
//!         .base_url("server_url")
//!         .app_version("MyApp@0.1.1")
//!         .build::<T>().unwrap();
//!
//!     let session = DefaultSession::refresh_async(&client, &user_uid, &user_refresh_token).await.unwrap();
//!
//!     // session is now authenticated and can access the rest of the API.
//!     // ...
//!
//!     session.logout_async(&client).await.unwrap();
//! }
//! ```

pub mod clientv2;
pub mod domain;
pub mod http;
mod requests;

pub use clientv2::*;

// Re-export tokio and log.
pub use log;
