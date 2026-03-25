use std::collections::HashMap;

/// Map from original server name → suffixed server name.
pub type RenameMap = HashMap<String, String>;

/// Rewrites permission entries to use suffixed server names.
///
/// Handles:
/// - Exact match: `mcp__postgres__query` → `mcp__postgres__jig_a3f1b2c9__query`
/// - Glob: `mcp__postgres__*` → `mcp__postgres__jig_a3f1b2c9__*`
pub fn rewrite_mcp_permissions(permissions: &[String], rename_map: &RenameMap) -> Vec<String> {
    permissions
        .iter()
        .map(|perm| apply_rename_to_permission(perm, rename_map))
        .collect()
}

fn apply_rename_to_permission(perm: &str, rename_map: &RenameMap) -> String {
    // Pattern: mcp__<server>__<rest>
    if !perm.starts_with("mcp__") {
        return perm.to_owned();
    }

    let without_prefix = &perm["mcp__".len()..];
    // Find the second __ separator
    if let Some(sep_pos) = without_prefix.find("__") {
        let server_name = &without_prefix[..sep_pos];
        let rest = &without_prefix[sep_pos..]; // includes the __ prefix

        if let Some(new_name) = rename_map.get(server_name) {
            // new_name is like "postgres__jig_a3f1b2c9"
            return format!("mcp__{new_name}{rest}");
        }
    }

    perm.to_owned()
}

/// Allowlist of claude CLI flags that jig permits in `claude_flags` passthrough.
/// Flags that jig manages itself are excluded to prevent conflicts.
pub const ALLOWED_PASSTHROUGH_FLAGS: &[&str] = &[
    "--verbose",
    "--output-format",
    "--max-turns",
    "--debug",
];

/// Validates passthrough claude_flags against the allowlist.
/// Returns a list of rejected flags.
pub fn validate_passthrough_flags(flags: &[String]) -> Vec<String> {
    flags
        .iter()
        .filter(|flag| {
            let flag_base = flag.split('=').next().unwrap_or(flag);
            !ALLOWED_PASSTHROUGH_FLAGS.contains(&flag_base)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_exact_match() {
        let mut rename_map = RenameMap::new();
        rename_map.insert("postgres".to_owned(), "postgres__jig_a3f1b2c9".to_owned());

        let input = vec!["mcp__postgres__query".to_owned()];
        let result = rewrite_mcp_permissions(&input, &rename_map);
        assert_eq!(result, vec!["mcp__postgres__jig_a3f1b2c9__query".to_owned()]);
    }

    #[test]
    fn test_rewrite_glob() {
        let mut rename_map = RenameMap::new();
        rename_map.insert("postgres".to_owned(), "postgres__jig_a3f1b2c9".to_owned());

        let input = vec!["mcp__postgres__*".to_owned()];
        let result = rewrite_mcp_permissions(&input, &rename_map);
        assert_eq!(result, vec!["mcp__postgres__jig_a3f1b2c9__*".to_owned()]);
    }

    #[test]
    fn test_no_match_unchanged() {
        let rename_map = RenameMap::new();
        let input = vec!["mcp__other__tool".to_owned()];
        let result = rewrite_mcp_permissions(&input, &rename_map);
        assert_eq!(result, input);
    }

    #[test]
    fn test_non_mcp_permission_unchanged() {
        let rename_map = RenameMap::new();
        let input = vec!["Bash".to_owned(), "Edit".to_owned()];
        let result = rewrite_mcp_permissions(&input, &rename_map);
        assert_eq!(result, input);
    }
}
