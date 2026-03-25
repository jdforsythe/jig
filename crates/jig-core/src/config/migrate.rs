/// Current schema version.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Returns true if the schema version requires migration.
pub fn needs_migration(version: u32) -> bool {
    version < CURRENT_SCHEMA_VERSION
}

/// Returns migration instructions for a schema version.
pub fn migration_message(version: u32) -> String {
    format!(
        "Config schema version {} is outdated (current: {}). \
         A backup will be created before migration. Run `jig doctor --migrate` to upgrade.",
        version, CURRENT_SCHEMA_VERSION
    )
}
