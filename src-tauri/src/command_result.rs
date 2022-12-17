use serde::Serialize;
use tracing::error;

#[allow(dead_code)]
pub type Result<T> = core::result::Result<T, CommandError>;

#[derive(Debug, Serialize)]
pub struct CommandError(pub String);

impl From<tauri::api::Error> for CommandError {
    fn from(e: tauri::api::Error) -> Self {
        error!("{}", e);
        Self(e.to_string())
    }
}
