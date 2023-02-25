use serde::Deserialize;

#[doc(hidden)]
#[derive(Deserialize)]
pub struct LatestEventResponse {
    #[serde(rename = "EventID")]
    pub event_id: crate::domain::EventId,
}
