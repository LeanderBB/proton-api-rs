use crate::domain::User;
use crate::http;
use crate::http::{JsonResponse, RequestFactory};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserInfoResponse {
    pub user: User,
}

pub struct UserInfoRequest {}

impl http::Request for UserInfoRequest {
    type Output = UserInfoResponse;
    type Response = JsonResponse<Self::Output>;

    fn build_request(&self, factory: &dyn RequestFactory) -> http::RequestData {
        factory.new_request(http::Method::Get, "core/v4/users")
    }
}
