use crate::domain::User;
use crate::http;
use crate::http::RequestFactory;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserInfoResponse {
    pub user: User,
}

pub struct UserInfoRequest {}

impl http::RequestWithBody for UserInfoRequest {
    type Response = UserInfoResponse;

    fn build_request(&self, factory: &dyn RequestFactory) -> http::Request {
        factory.new_request(http::Method::Get, "core/v4/users")
    }
}
