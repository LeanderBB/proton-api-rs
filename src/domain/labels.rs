use crate::domain::Boolean;
use serde::Deserialize;
use serde_repr::Deserialize_repr;
use std::fmt::{Display, Formatter};

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
/// Id for a proton Label.
pub struct LabelId(pub String);

impl Display for LabelId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Deserialize_repr, Eq, PartialEq, Copy, Clone)]
#[repr(u8)]
pub enum LabelType {
    Label = 0,
    ContactGroup = 1,
    Folder = 2,
    System = 3,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Label {
    #[serde(rename = "ID")]
    pub id: LabelId,
    #[serde(rename = "ParentID")]
    pub parent_id: Option<LabelId>,
    pub name: String,
    pub path: String,
    pub color: String,
    #[serde(rename = "Type")]
    pub label_type: LabelType,
    #[serde(default)]
    pub notify: Boolean,
    #[serde(default)]
    pub display: Boolean,
    #[serde(default)]
    pub sticky: Boolean,
    #[serde(default)]
    pub expanded: Boolean,
    #[serde(default = "default_label_order")]
    pub order: i32,
}

fn default_label_order() -> i32 {
    0
}
