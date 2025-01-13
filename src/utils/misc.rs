use std::error::Error as StdError;

use anyhow::anyhow;

pub fn sendable_anyhow(msg: String) -> Box<dyn StdError + Send> {
    anyhow!(msg).into()
}
