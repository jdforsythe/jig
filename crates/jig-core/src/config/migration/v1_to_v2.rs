use super::{Migration, MigrationError};
use serde_json::Value;

pub struct V1ToV2;

impl Migration for V1ToV2 {
    fn from_version(&self) -> u32 {
        1
    }
    fn to_version(&self) -> u32 {
        2
    }
    fn description(&self) -> &'static str {
        "Upgrade schema from v1 to v2 (bump schema version field)"
    }

    fn migrate(&self, mut doc: Value) -> Result<(Value, Vec<String>), MigrationError> {
        let changes = vec!["schema: 1 → 2".to_owned()];
        if let Some(obj) = doc.as_object_mut() {
            obj.insert("schema".to_owned(), Value::Number(2.into()));
        }
        Ok((doc, changes))
    }
}
