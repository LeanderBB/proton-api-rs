use crate::http::{Request, RequestDesc};
use crate::requests::{CaptchaRequest, Ping};

pub fn ping() -> impl Request {
    Ping.to_request()
}

pub fn captcha_get(token: &str, force_web: bool) -> impl Request {
    CaptchaRequest::new(token, force_web).to_request()
}
