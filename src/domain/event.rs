use serde::Deserialize;
use serde_repr::Deserialize_repr;
use std::fmt::{Display, Formatter};

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
/// Id for an API Event.
pub struct EventId(pub String);

impl Display for EventId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Deserialize_repr, Eq, PartialEq, Copy, Clone)]
#[repr(u8)]
pub enum MoreEvents {
    No = 0,
    Yes = 1,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Event {
    #[serde(rename = "EventID")]
    pub event_id: EventId,
    pub more: MoreEvents,
    pub messages: Option<Vec<MessageEvent>>,
}

#[derive(Debug, Deserialize_repr, Eq, PartialEq, Copy, Clone)]
#[repr(u8)]
pub enum MessageAction {
    Delete = 0,
    Create = 1,
    Update = 2,
    UpdateFlags = 3,
}

/// Message API ID.
#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
pub struct MessageId(String);

/// Labels API ID. Note that label IDs are used interchangeably between what we would consider
/// mail labels and mailboxes.
#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
pub struct LabelID(String);

impl LabelID {
    /// Default LabelID for the `INBOX` mailbox.
    pub fn inbox() -> Self {
        Self("0".to_string())
    }
}

/// Event data related to a Message event.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MessageEvent {
    #[serde(rename = "ID")]
    pub id: MessageId,
    pub action: MessageAction,
    pub message: Message,
}

/// Represents an email message.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Message {
    #[serde(rename = "ID")]
    pub id: MessageId,
    #[serde(rename = "LabelIDs")]
    pub labels: Vec<LabelID>,
}
