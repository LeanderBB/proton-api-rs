use proton_api_rs::clientv2::{ping, SessionType};
use proton_api_rs::domain::CaptchaErrorDetail;
use proton_api_rs::{captcha_get, http, LoginError, Session};
use std::io::{BufRead, Write};

fn main() {
    env_logger::init();

    let user_email = std::env::var("PAPI_USER_EMAIL").unwrap();
    let user_password = std::env::var("PAPI_USER_PASSWORD").unwrap();
    let app_version = std::env::var("PAPI_APP_VERSION").unwrap();

    let client = http::ClientBuilder::new()
        .app_version(&app_version)
        .build::<http::ureq_client::UReqClient>()
        .unwrap();

    ping(&client).unwrap();

    let login_result = Session::login(&client, &user_email, &user_password, None);
    if let Err(LoginError::Request(http::Error::API(e))) = &login_result {
        if e.api_code != 9001 {
            panic!("{e}")
        }
        let captcha_desc =
            serde_json::from_value::<CaptchaErrorDetail>(e.details.clone().unwrap()).unwrap();

        let captcha_body = captcha_get(&client, &captcha_desc.human_verification_token).unwrap();
        run_captcha(captcha_body);
        // TODO: Start webview with the downloaded body - use https://github.com/tauri-apps/wry
        // Click
        // Handle postMessageToParent which has token & token type
        // repeat submission with x-pm-human-verification-token and x-pm-human-verification-token-type
        // Use the event below to catch this
        // window.addEventListener(
        //   "message",
        //   (event) => {
        //      -> event.Data
        //   },
        //   false
        // );
        // On Success
        // postMessageToParent({
        //                 "type": "pm_captcha",
        //                 "token": response
        //             });
        //
        // on expired
        // postMessageToParent({
        //                 "type": "pm_captcha_expired",
        //                 "token": response
        //             });
        //
        // on height:
        // postMessageToParent({
        //                 'type': 'pm_height',
        //                 'height': height
        //             });
        return;
    }

    let session = match login_result.unwrap() {
        SessionType::Authenticated(s) => s,
        SessionType::AwaitingTotp(mut t) => {
            let mut line_reader = std::io::BufReader::new(std::io::stdin());
            let session = {
                let mut session = None;
                for _ in 0..3 {
                    std::io::stdout()
                        .write_all("Please Input TOTP:".as_bytes())
                        .unwrap();
                    std::io::stdout().flush().unwrap();

                    let mut line = String::new();
                    if let Err(e) = line_reader.read_line(&mut line) {
                        eprintln!("Failed to read totp {e}");
                        return;
                    };

                    let totp = line.trim_end_matches('\n');

                    match t.submit_totp(&client, totp) {
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

    let user = session.get_user(&client).unwrap();
    println!("User ID is {}", user.id);

    session.logout(&client).unwrap();
}

fn run_captcha(html: String) {
    std::fs::write("/tmp/captcha.html", html).unwrap();
    use wry::{
        application::{
            event::{Event, StartCause, WindowEvent},
            event_loop::{ControlFlow, EventLoop},
            window::WindowBuilder,
        },
        webview::WebViewBuilder,
    };

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Proton API Captcha")
        .build(&event_loop)
        .unwrap();
    let _webview = WebViewBuilder::new(window)
        .unwrap()
        .with_url("http://127.0.0.1:8000/captcha.html")
        .unwrap()
        .with_devtools(true)
        .with_ipc_handler(|window, req| {
            println!("Window IPC: {req}");
        })
        .build()
        .unwrap();

    _webview
        .evaluate_script(
            "postMessageToParent = function(message) { window.ipc.postMessage(JSON.stringify(message), '*')}",
        )
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("Close requested");
                *control_flow = ControlFlow::Exit
            }
            _ => (),
        }
    });
}
