use crate::http;

pub struct Ping;

impl http::Request for Ping {
    type Output = ();
    type Response = http::NoResponse;

    fn build_request(&self, factory: &dyn http::RequestFactory) -> http::RequestData {
        factory.new_request(http::Method::Get, "tests/ping")
    }
}
