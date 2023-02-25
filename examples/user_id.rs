use proton_api_rs::{tokio, ClientBuilder, ClientLoginState};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

#[tokio::main(worker_threads = 1)]
async fn main() {
    let user_email = std::env::var("PAPI_USER_EMAIL").unwrap();
    let user_password = std::env::var("PAPI_USER_PASSWORD").unwrap();
    let app_version = std::env::var("PAPI_APP_VERSION").unwrap();

    let client = match ClientBuilder::new()
        .app_version(&app_version)
        .login(&user_email, &user_password)
        .await
        .unwrap()
    {
        ClientLoginState::Authenticated(c) => c,

        ClientLoginState::AwaitingTotp(mut t) => {
            let mut stdout = tokio::io::stdout();
            let mut line_reader = tokio::io::BufReader::new(tokio::io::stdin()).lines();
            let client = {
                let mut client = None;
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

                    match t.submit_totp(totp).await {
                        Ok(ac) => {
                            client = Some(ac);
                            break;
                        }
                        Err((et, e)) => {
                            t = et;
                            eprintln!("Failed to submit totp: {e}");
                            continue;
                        }
                    }
                }

                client
            };

            let Some(c) = client else {
                eprintln!("Failed to pass TOTP 2FA auth");
                return;
            };
            c
        }
    };

    let user = client.get_user().await.unwrap();
    println!("User ID is {}", user.id);

    client.logout().await.unwrap();
}
