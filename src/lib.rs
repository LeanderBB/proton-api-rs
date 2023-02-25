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
//! Login into a new session:
//! ```
//! use proton_api_rs::{ClientBuilder, ClientLoginState};
//! async fn example() {
//!     let client = ClientBuilder::new()
//!         .user_agent("MyUserAgent/0.0.0")
//!         .base_url("server_url")
//!         .app_version("MyApp@0.1.1")
//!         .login("my_address@proton.me", "my_proton_password").await.unwrap();
//!
//!
//!     let client = match client {
//!         // Client is authenticated, no 2FA verifications necessary.
//!         ClientLoginState::Authenticated(c) => c,
//!         // Client needs 2FA TOTP auth.
//!         ClientLoginState::AwaitingTotp(t) => {
//!             t.submit_totp("000000").await.unwrap()
//!         }
//!     };
//!
//!     // client is now authenticated and can access the rest of the API.
//!     // ...
//!     client.logout().await.unwrap();
//! }
//! ```
//!
//! Login using a previous sessions token.
//! ```
//! use proton_api_rs::{ClientBuilder, ClientLoginState};
//! use proton_api_rs::domain::UserUid;
//!
//! async fn example() {
//!     let user_uid = "user_uid".into();
//!     let user_refresh_token = "token";
//!     let client = ClientBuilder::new()
//!         .user_agent("MyUserAgent/0.0.0")
//!         .base_url("server_url")
//!         .app_version("MyApp@0.1.1")
//!         .with_token(&user_uid, &user_refresh_token).await.unwrap();
//!
//!     // client is now authenticated and can access the rest of the API.
//!     // ...
//!     client.logout().await.unwrap();
//! }
//! ```

mod client;
pub mod domain;
pub use client::*;

// Re-export tokio and log.
pub use log;
pub use tokio;
