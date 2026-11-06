//! One error type for every Tauri command: any error becomes a readable string for the UI.
use serde::{Serialize, Serializer};

pub struct CmdError(pub String);

impl Serialize for CmdError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<E: Into<anyhow::Error>> From<E> for CmdError {
    fn from(e: E) -> Self {
        CmdError(format!("{:#}", e.into()))
    }
}

pub type CmdResult<T> = Result<T, CmdError>;
