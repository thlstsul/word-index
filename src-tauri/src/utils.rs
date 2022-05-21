use std::error::Error;

use tracing::error;

pub fn union_err(err: impl Error) -> String {
    error!("{}", err);
    format!("{}", err)
}
