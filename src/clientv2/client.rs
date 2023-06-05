use crate::http;
use crate::http::Request;
use crate::requests::{CaptchaRequest, Ping};

pub fn ping<T: http::ClientSync>(client: &T) -> Result<(), http::Error> {
    Ping.execute_sync::<T>(client, &http::DefaultRequestFactory {})
}

pub async fn ping_async<T: http::ClientAsync>(client: &T) -> Result<(), http::Error> {
    Ping.execute_async::<T>(client, &http::DefaultRequestFactory {})
        .await
}

pub fn captcha_get<T: http::ClientSync>(
    client: &T,
    token: &str,
    force_web: bool,
) -> Result<String, http::Error> {
    CaptchaRequest::new(token, force_web).execute_sync(client, &http::DefaultRequestFactory {})
}

pub async fn captcha_get_async<T: http::ClientAsync>(
    client: &T,
    token: &str,
    force_web: bool,
) -> Result<String, http::Error> {
    CaptchaRequest::new(token, force_web)
        .execute_async(client, &http::DefaultRequestFactory {})
        .await
}
