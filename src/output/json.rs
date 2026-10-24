use anyhow::Result;

use crate::model::Inventory;

pub fn render(inv: &Inventory) -> Result<String> {
    Ok(serde_json::to_string_pretty(inv)?)
}
