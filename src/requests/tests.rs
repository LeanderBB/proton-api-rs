use crate::http;

pub struct Ping;

impl http::RequestNoBody for Ping {
    fn build_request(&self, factory: &dyn http::RequestFactory) -> http::Request {
        factory.new_request(http::Method::Get, "tests/ping")
    }
}
