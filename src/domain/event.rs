use crate::domain::Boolean;
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

impl Display for MessageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Labels API ID. Note that label IDs are used interchangeably between what we would consider
/// mail labels and mailboxes.
#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
pub struct LabelID(String);

/// SysLabelID represents system label identifiers that are constant for every account.
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub struct SysLabelID(&'static str);

impl PartialEq<LabelID> for SysLabelID {
    fn eq(&self, other: &LabelID) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<SysLabelID> for LabelID {
    fn eq(&self, other: &SysLabelID) -> bool {
        self.0 == other.0
    }
}

impl From<SysLabelID> for LabelID {
    fn from(value: SysLabelID) -> Self {
        Self(value.0.into())
    }
}

impl SysLabelID {
    pub const INBOX: SysLabelID = SysLabelID("0");
    pub const ALL_DRAFTS: SysLabelID = SysLabelID("1");
    pub const ALL_SENT: SysLabelID = SysLabelID("1");
    pub const TRASH: SysLabelID = SysLabelID("3");
    pub const SPAM: SysLabelID = SysLabelID("4");
    pub const ALL_MAIL: SysLabelID = SysLabelID("5");
    pub const ARCHIVE: SysLabelID = SysLabelID("5");
    pub const SENT: SysLabelID = SysLabelID("7");
    pub const DRAFTS: SysLabelID = SysLabelID("8");
    pub const OUTBOX: SysLabelID = SysLabelID("9");
    pub const STARRED: SysLabelID = SysLabelID("10");
    pub const ALL_SCHEDULED: SysLabelID = SysLabelID("12");
}

impl LabelID {
    pub fn inbox() -> Self {
        SysLabelID::INBOX.into()
    }

    pub fn all_drafts() -> Self {
        SysLabelID::ALL_DRAFTS.into()
    }

    pub fn all_sent() -> Self {
        SysLabelID::ALL_SENT.into()
    }

    pub fn trash() -> Self {
        SysLabelID::TRASH.into()
    }

    pub fn spam() -> Self {
        SysLabelID::SPAM.into()
    }

    pub fn all_mail() -> Self {
        SysLabelID::ALL_MAIL.into()
    }

    pub fn archive() -> Self {
        SysLabelID::ARCHIVE.into()
    }

    pub fn sent() -> Self {
        SysLabelID::SENT.into()
    }

    pub fn drafts() -> Self {
        SysLabelID::DRAFTS.into()
    }

    pub fn outbox() -> Self {
        SysLabelID::OUTBOX.into()
    }

    pub fn starred() -> Self {
        SysLabelID::STARRED.into()
    }

    pub fn all_scheduled() -> Self {
        SysLabelID::ALL_SCHEDULED.into()
    }
}

impl Display for LabelID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Display for SysLabelID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Event data related to a Message event.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MessageEvent {
    #[serde(rename = "ID")]
    pub id: MessageId,
    pub action: MessageAction,
    pub message: Option<Message>,
}

/// Represents an email message.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Message {
    #[serde(rename = "ID")]
    pub id: MessageId,
    #[serde(rename = "LabelIDs")]
    pub labels: Vec<LabelID>,
    pub subject: String,
    pub sender_address: String,
    pub sender_name: Option<String>,
    pub unread: Boolean,
}
