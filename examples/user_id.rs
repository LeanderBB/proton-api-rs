use proton_api_rs::{http, ping_async};
use proton_api_rs::{Session, SessionType};
pub use tokio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

#[tokio::main(worker_threads = 1)]
async fn main() {
    let user_email = std::env::var("PAPI_USER_EMAIL").unwrap();
    let user_password = std::env::var("PAPI_USER_PASSWORD").unwrap();
    let app_version = std::env::var("PAPI_APP_VERSION").unwrap();

    let client = http::ClientBuilder::new()
        .app_version(&app_version)
        .build::<http::reqwest_client::ReqwestClient>()
        .unwrap();

    ping_async(&client).await.unwrap();

    let session = match Session::login_async(&client, &user_email, &user_password, None)
        .await
        .unwrap()
    {
        SessionType::Authenticated(c) => c,

        SessionType::AwaitingTotp(mut t) => {
            let mut stdout = tokio::io::stdout();
            let mut line_reader = tokio::io::BufReader::new(tokio::io::stdin()).lines();
            let session = {
                let mut session = None;
                for _ in 0..3 {
                    stdout
                        .write_all("Please Input TOTP:".as_bytes())
                        .await
                        .unwrap();
                    stdout.flush().await.unwrap();

                    let Some(line) = line_reader.next_line().await.unwrap() else {
                        eprintln!("Failed to read totp");
                        return;
                    };

                    let totp = line.trim_end_matches('\n');

                    match t.submit_totp_async(&client, totp).await {
                        Ok(ac) => {
                            session = Some(ac);
                            break;
                        }
                        Err((et, e)) => {
                            t = et;
                            eprintln!("Failed to submit totp: {e}");
                            continue;
                        }
                    }
                }

                session
            };

            let Some(c) = session else {
                eprintln!("Failed to pass TOTP 2FA auth");
                return;
            };
            c
        }
    };

    let user = session.get_user_async(&client).await.unwrap();
    println!("User ID is {}", user.id);

    session.logout_async(&client).await.unwrap();
}
