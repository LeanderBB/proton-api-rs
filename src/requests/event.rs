use crate::http;
use crate::http::{RequestData, RequestFactory};
use serde::Deserialize;

#[doc(hidden)]
#[derive(Deserialize)]
pub struct LatestEventResponse {
    #[serde(rename = "EventID")]
    pub event_id: crate::domain::EventId,
}

pub struct GetLatestEventRequest;

impl http::Request for GetLatestEventRequest {
    type Output = LatestEventResponse;
    type Response = http::JsonResponse<Self::Output>;

    fn build_request(&self, factory: &dyn RequestFactory) -> RequestData {
        factory.new_request(http::Method::Get, "core/v4/events/latest")
    }
}

pub struct GetEventRequest<'a> {
    event_id: &'a crate::domain::EventId,
}

impl<'a> GetEventRequest<'a> {
    pub fn new(id: &'a crate::domain::EventId) -> Self {
        Self { event_id: id }
    }
}

impl<'a> http::Request for GetEventRequest<'a> {
    type Output = crate::domain::Event;
    type Response = http::JsonResponse<Self::Output>;

    fn build_request(&self, factory: &dyn RequestFactory) -> RequestData {
        factory.new_request(
            http::Method::Get,
            &format!("core/v4/events/{}", self.event_id),
        )
    }
}
