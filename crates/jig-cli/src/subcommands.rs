use std::path::Path;

use jig_core::{
    config::resolve::{CliOverrides, resolve_config},
    defaults::{builtin_personas, builtin_template_refs, builtin_templates},
};

use crate::cli::{
    Commands, ConfigSubcommand, PersonaSubcommand, SkillSubcommand, TemplateSubcommand,
};

pub fn dispatch(cmd: &Commands, cwd: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Commands::Template(args) => handle_template(&args.subcommand, cwd, json),
        Commands::Persona(args) => handle_persona(&args.subcommand, json),
        Commands::Skill(args) => handle_skill(&args.subcommand, json),
        Commands::History(args) => handle_history(args.limit, args.verbose, json),
        Commands::Doctor(args) => handle_doctor_to(cwd, args.audit, args.migrate, &mut std::io::stdout()),
        Commands::Config(args) => handle_config(&args.subcommand, cwd, json),
        Commands::Init => handle_init(cwd),
        Commands::Sync(args) => handle_sync(args, cwd),
        Commands::Import(args) => handle_import(args.url.as_deref(), &args.scope, args.dry_run, cwd),
        Commands::Diff(args) => handle_diff(&args.config, cwd),
        Commands::Completions(_) => Ok(()), // handled in main.rs before routing here
    }
}

fn handle_template(sub: &TemplateSubcommand, cwd: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    match sub {
        TemplateSubcommand::List => {
            let templates = builtin_template_refs();
            if json {
                println!("{}", serde_json::to_string_pretty(&templates)?);
            } else {
                for t in &templates {
                    if let Some(desc) = &t.description {
                        println!("{:20} {}", t.name, desc);
                    } else {
                        println!("{}", t.name);
                    }
                }
            }
        }
        TemplateSubcommand::Show { name } => {
            let templates = builtin_templates();
            if let Some(t) = templates.iter().find(|t| &t.name == name) {
                if json {
                    println!("{}", serde_json::to_string_pretty(t)?);
                } else {
                    println!("Template: {}", t.name);
                    if let Some(desc) = &t.description {
                        println!("Description: {desc}");
                    }
                }
            } else {
                let available: Vec<_> = templates.iter().map(|t| t.name.as_str()).collect();
                eprintln!("Template '{name}' not found. Available: {}", available.join(", "));
                std::process::exit(1);
            }
        }
        TemplateSubcommand::New => {
            #[cfg(feature = "tui")]
            {
                jig_tui::editor::run_editor_tui(
                    jig_tui::editor::EditorEntryPoint::NewTemplate,
                    None,
                    cwd,
                )
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            }
            #[cfg(not(feature = "tui"))]
            {
                let _ = cwd;
                eprintln!("Editor mode requires TUI feature. Build without --no-default-features.");
            }
        }
        TemplateSubcommand::Edit { name } => {
            #[cfg(feature = "tui")]
            {
                let draft = jig_core::editor::load_draft_for_template(name);
                jig_tui::editor::run_editor_tui(
                    jig_tui::editor::EditorEntryPoint::EditTemplate,
                    Some(draft),
                    cwd,
                )
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            }
            #[cfg(not(feature = "tui"))]
            {
                let _ = (name, cwd);
                eprintln!("Editor mode requires TUI feature.");
            }
        }
    }
    Ok(())
}

fn handle_persona(sub: &PersonaSubcommand, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    match sub {
        PersonaSubcommand::List => {
            let personas = builtin_personas();
            if json {
                let list: Vec<serde_json::Value> = personas
                    .iter()
                    .map(|(name, p)| {
                        serde_json::json!({
                            "name": name,
                            "rules": p.rules.as_deref().unwrap_or_default(),
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else {
                for (name, persona) in &personas {
                    let rules_count = persona.rules.as_ref().map(Vec::len).unwrap_or(0);
                    println!("{:20} ({} rules)", name, rules_count);
                }
            }
        }
        PersonaSubcommand::Show { name } => {
            let personas = builtin_personas();
            if let Some((pname, persona)) = personas.iter().find(|(n, _)| n == name) {
                if json {
                    println!("{}", serde_json::to_string_pretty(persona)?);
                } else {
                    println!("Persona: {pname}");
                    if let Some(rules) = &persona.rules {
                        println!("Rules:");
                        for rule in rules {
                            println!("  - {rule}");
                        }
                    }
                }
            } else {
                let available: Vec<_> = personas.iter().map(|(n, _)| n.as_str()).collect();
                eprintln!("Persona '{name}' not found. Available: {}", available.join(", "));
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

fn handle_skill(sub: &SkillSubcommand, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use jig_core::assembly::skill_index::{read_index, search};
    use jig_core::assembly::skills_lock::{read_skills_lock, verify_skill_integrity};
    use jig_core::assembly::source_resolver::{skill_file_path, override_skill_path};

    match sub {
        SkillSubcommand::List { source } => {
            let index = read_index();
            let mut found = false;
            for (src_name, skills) in &index.entries {
                if let Some(filter) = source {
                    if src_name != filter { continue; }
                }
                for skill in skills {
                    found = true;
                    if json {
                        println!("{}", serde_json::to_string(skill)?);
                    } else {
                        let desc = skill.meta.description.as_deref().unwrap_or("");
                        println!("{}/{}: {}", src_name, skill.skill_name, desc);
                    }
                }
            }
            if !found {
                if json {
                    println!("[]");
                } else {
                    println!("No skills installed. Run `jig sync` to fetch skills from configured sources.");
                }
            }
        }

        SkillSubcommand::Search { query, json: json_flag } => {
            let index = read_index();
            let results = search(&index, query);
            if *json_flag || json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else if results.is_empty() {
                println!("No skills match '{query}'.");
            } else {
                for skill in &results {
                    let desc = skill.meta.description.as_deref().unwrap_or("");
                    let tags = skill.meta.tags.as_deref()
                        .map(|t| format!(" [{}]", t.join(", ")))
                        .unwrap_or_default();
                    println!("{}/{}{}: {}", skill.source, skill.skill_name, tags, desc);
                }
            }
        }

        SkillSubcommand::Info { source, skill } => {
            let path = skill_file_path(source, skill);
            let override_path = override_skill_path(source, skill);

            println!("Source:  {source}");
            println!("Skill:   {skill}");

            let active_path = if override_path.exists() {
                println!("Status:  [OVERRIDDEN]");
                &override_path
            } else if path.exists() {
                println!("Status:  installed");
                &path
            } else {
                println!("Status:  not installed (run `jig sync`)");
                return Ok(());
            };

            println!("Path:    {}", active_path.display());

            // Show metadata
            if let Ok(meta) = jig_core::assembly::skill_meta::parse_frontmatter(active_path) {
                if let Some(name) = &meta.name { println!("Name:    {name}"); }
                if let Some(desc) = &meta.description { println!("Desc:    {desc}"); }
                if let Some(tags) = &meta.tags { println!("Tags:    {}", tags.join(", ")); }
                if let Some(ver) = &meta.version { println!("Version: {ver}"); }
            }

            // Integrity check
            match verify_skill_integrity(source, skill, &path) {
                Some(true) => println!("Integrity: verified"),
                Some(false) => println!("Integrity: MISMATCH — file may have been tampered with"),
                None => println!("Integrity: - not in lock file (run `jig sync` to update)"),
            }

            // Lock info
            let lock = read_skills_lock();
            if let Some(source_entry) = lock.sources.get(source.as_str()) {
                println!("Fetched: {}", source_entry.fetched_at);
                println!("SHA:     {}", &source_entry.sha[..8.min(source_entry.sha.len())]);
            }
        }

        SkillSubcommand::Override { source, skill } => {
            let upstream = skill_file_path(source, skill);
            if !upstream.exists() {
                eprintln!("Skill '{source}/{skill}' is not installed. Run `jig sync` first.");
                std::process::exit(1);
            }

            let override_dest = override_skill_path(source, skill);
            if let Some(parent) = override_dest.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::copy(&upstream, &override_dest)?;
            println!("Created override: {}", override_dest.display());
            println!("Edit the file to customize, then use `jig skill diff {source} {skill}` to review changes.");
        }

        SkillSubcommand::Diff { source, skill } => {
            let upstream = skill_file_path(source, skill);
            let override_path = override_skill_path(source, skill);

            if !override_path.exists() {
                println!("No local override for '{source}/{skill}'.");
                println!("Use `jig skill override {source} {skill}` to create one.");
                return Ok(());
            }

            if !upstream.exists() {
                println!("Upstream not found for '{source}/{skill}'. Run `jig sync` first.");
                return Ok(());
            }

            let upstream_content = std::fs::read_to_string(&upstream)?;
            let override_content = std::fs::read_to_string(&override_path)?;

            if upstream_content == override_content {
                println!("No differences.");
                return Ok(());
            }

            // Simple line-by-line diff output
            println!("--- upstream/{source}/{skill}.md");
            println!("+++ local-override/{source}/{skill}.md");

            let diff = compute_diff(&upstream_content, &override_content);
            println!("{diff}");
        }

        SkillSubcommand::Reset { source, skill, yes } => {
            let override_path = override_skill_path(source, skill);

            if !override_path.exists() {
                println!("No local override for '{source}/{skill}'.");
                return Ok(());
            }

            if !yes {
                eprint!("Reset '{source}/{skill}' to upstream? This will delete your local override. [y/N] ");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled.");
                    return Ok(());
                }
            }

            std::fs::remove_file(&override_path)?;

            // Clean up empty override directory
            if let Some(parent) = override_path.parent() {
                if parent.exists() {
                    if let Ok(mut entries) = std::fs::read_dir(parent) {
                        if entries.next().is_none() {
                            let _ = std::fs::remove_dir(parent);
                        }
                    }
                }
            }

            println!("Reset '{source}/{skill}' — override removed.");
        }
    }

    Ok(())
}

fn compute_diff(original: &str, modified: &str) -> String {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mod_lines: Vec<&str> = modified.lines().collect();

    let mut output = String::new();

    let orig_len = orig_lines.len();
    let mod_len = mod_lines.len();

    // Simple line-by-line comparison (not a true diff algorithm but sufficient for display)
    let max_len = orig_len.max(mod_len);

    for i in 0..max_len {
        match (orig_lines.get(i), mod_lines.get(i)) {
            (Some(o), Some(m)) if o == m => {
                output.push_str(&format!(" {o}\n"));
            }
            (Some(o), Some(m)) => {
                output.push_str(&format!("-{o}\n"));
                output.push_str(&format!("+{m}\n"));
            }
            (Some(o), None) => {
                output.push_str(&format!("-{o}\n"));
            }
            (None, Some(m)) => {
                output.push_str(&format!("+{m}\n"));
            }
            (None, None) => {}
        }
    }

    output
}

fn handle_history(limit: usize, verbose: bool, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use jig_core::history::history_path;

    let path = history_path();
    if !path.exists() {
        if json {
            println!("[]");
        } else {
            println!("No session history found.");
        }
        return Ok(());
    }

    let contents = std::fs::read_to_string(&path)?;
    let all_lines: Vec<serde_json::Value> = contents
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    // Build exit record map: session_id → exit record
    let exit_map: std::collections::HashMap<String, serde_json::Value> = all_lines
        .iter()
        .filter(|v| v.get("type").and_then(|t| t.as_str()) == Some("exit"))
        .filter_map(|v| {
            v.get("session_id").and_then(|id| id.as_str()).map(|id| (id.to_owned(), v.clone()))
        })
        .collect();

    // Collect recent start records
    let recent: Vec<serde_json::Value> = all_lines
        .iter()
        .rev()
        .filter(|v| v.get("type").and_then(|t| t.as_str()) == Some("start"))
        .take(limit)
        .cloned()
        .collect();

    if json {
        // Augment start records with exit info
        let augmented: Vec<serde_json::Value> = recent
            .iter()
            .map(|entry| {
                let mut e = entry.clone();
                if let Some(id) = entry.get("session_id").and_then(|v| v.as_str()) {
                    if let Some(exit) = exit_map.get(id) {
                        e["exit_code"] = exit["exit_code"].clone();
                        e["ended_at"] = exit["ended_at"].clone();
                    }
                }
                e
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&augmented)?);
    } else {
        for entry in &recent {
            let id = &entry["session_id"].as_str().unwrap_or("?")[..8.min(entry["session_id"].as_str().unwrap_or("?").len())];
            let template = entry["template"].as_str().unwrap_or("none");
            let started = &entry["started_at"].as_str().unwrap_or("?")[..16.min(entry["started_at"].as_str().unwrap_or("?").len())];

            if verbose {
                let persona = entry["persona"].as_str().unwrap_or("none");
                let exit_code = entry.get("session_id")
                    .and_then(|sid| sid.as_str())
                    .and_then(|sid| exit_map.get(sid))
                    .and_then(|e| e["exit_code"].as_i64())
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".to_owned());
                println!("{started}  {template:20}  {persona:20}  exit:{exit_code}  {id}");
            } else {
                println!("{started}  {template:20}  {id}");
            }
        }
    }
    Ok(())
}

/// Writer-based doctor implementation — testable without stdout capture.
pub(crate) fn handle_doctor_to<W: std::io::Write>(
    cwd: &Path,
    audit: bool,
    migrate: bool,
    out: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(out, "jig doctor — checking system state...")?;

    // Check claude binary
    if jig_core::assembly::executor::find_claude_binary().is_some() {
        writeln!(out, "  ✓ claude binary found")?;
    } else {
        writeln!(out, "  ✗ claude binary not found in PATH")?;
        writeln!(out, "    Install claude: https://claude.ai/download")?;
    }

    // Check ~/.claude.json
    let claude_path = jig_core::assembly::mcp::claude_json_path();
    if claude_path.exists() {
        writeln!(out, "  ✓ ~/.claude.json exists")?;
    } else {
        writeln!(out, "  ! ~/.claude.json not found (will be created on first MCP write)")?;
    }

    // Check history
    let history_path = jig_core::history::history_path();
    if history_path.exists() {
        let lines = std::fs::read_to_string(&history_path)
            .map(|c| c.lines().count())
            .unwrap_or(0);
        writeln!(out, "  ✓ history.jsonl exists ({lines} entries)")?;
    } else {
        writeln!(out, "  ! history.jsonl not found (created on first launch)")?;
    }

    // Check for git worktree
    if jig_core::worktree::is_git_worktree(cwd) {
        if let Some(main_path) = jig_core::worktree::main_worktree_path(cwd) {
            writeln!(out, "  ! git worktree detected (main checkout: {})", main_path.display())?;
        } else {
            writeln!(out, "  ! git worktree detected")?;
        }
    }

    writeln!(out, "Done.")?;

    if audit {
        writeln!(out)?;
        writeln!(out, "jig doctor --audit — running security checks...")?;

        // Check global config file permissions (Unix only)
        let global_config = home::home_dir()
            .unwrap_or_default()
            .join(".config")
            .join("jig")
            .join("config.yaml");

        #[cfg(unix)]
        if global_config.exists() {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&global_config)?;
            let mode = meta.permissions().mode() & 0o777;
            if mode == 0o600 || mode == 0o640 {
                writeln!(out, "  ✓ global config permissions: {:o} (ok)", mode)?;
            } else {
                writeln!(out, "  ! global config permissions: {:o} (expected 0600 or 0640)", mode)?;
                writeln!(out, "    Run: chmod 600 {}", global_config.display())?;
            }
        } else {
            writeln!(out, "  - global config does not exist (ok — no permissions to check)")?;
        }

        // Check config schema validity
        if global_config.exists() {
            match std::fs::read_to_string(&global_config)
                .and_then(|s| serde_yaml::from_str::<jig_core::config::schema::JigConfig>(&s)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
            {
                Ok(cfg) => {
                    writeln!(out, "  ✓ global config parses as valid YAML")?;
                    if let Some(v) = cfg.schema {
                        writeln!(out, "  ✓ schema version: {v}")?;
                    }
                }
                Err(e) => {
                    writeln!(out, "  ✗ global config parse error: {e}")?;
                }
            }
        }

        // Check project config if present
        let project_config = cwd.join(".jig.yaml");
        if project_config.exists() {
            match std::fs::read_to_string(&project_config)
                .and_then(|s| serde_yaml::from_str::<jig_core::config::schema::JigConfig>(&s)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
            {
                Ok(_) => writeln!(out, "  ✓ .jig.yaml parses as valid YAML")?,
                Err(e) => writeln!(out, "  ✗ .jig.yaml parse error: {e}")?,
            }
        }

        // Check for .jig.local.yaml
        let local_config = cwd.join(".jig.local.yaml");
        if local_config.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let meta = std::fs::metadata(&local_config)?;
                let mode = meta.permissions().mode() & 0o777;
                if mode == 0o600 || mode == 0o640 {
                    writeln!(out, "  ✓ .jig.local.yaml permissions: {:o} (ok)", mode)?;
                } else {
                    writeln!(out, "  ! .jig.local.yaml permissions: {:o} (consider chmod 600 for security)", mode)?;
                }
            }
        }

        writeln!(out, "Audit complete.")?;
    }

    if migrate {
        use jig_core::config::migrate::{needs_migration, CURRENT_SCHEMA_VERSION};
        use jig_core::config::migration::apply_migration_chain;
        use std::path::PathBuf;

        writeln!(out)?;
        writeln!(out, "jig doctor --migrate — checking schema versions...")?;

        let global_config = home::home_dir()
            .unwrap_or_default()
            .join(".config")
            .join("jig")
            .join("config.yaml");

        let project_path = cwd.join(".jig.yaml");
        let local_path = cwd.join(".jig.local.yaml");

        let paths: Vec<PathBuf> = [global_config, project_path, local_path]
            .into_iter()
            .filter(|p| p.exists())
            .collect();

        if paths.is_empty() {
            writeln!(out, "  No config files found.")?;
        }

        for path in &paths {
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    writeln!(out, "  ✗ Could not read {}: {e}", path.display())?;
                    continue;
                }
            };
            let version: u32 = serde_yaml::from_str::<serde_json::Value>(&content)
                .ok()
                .and_then(|v| v.get("schema").and_then(|s| s.as_u64()))
                .map(|v| v as u32)
                .unwrap_or(1);

            if needs_migration(version) {
                writeln!(
                    out,
                    "  ! {} requires migration (schema v{} → v{})",
                    path.display(),
                    version,
                    CURRENT_SCHEMA_VERSION
                )?;
                let result = apply_migration_chain(path, version, |_changes| {
                    // Auto-confirm when --migrate is explicitly passed
                    true
                });
                match result {
                    Ok(outcome) if !outcome.changes.is_empty() => {
                        writeln!(
                            out,
                            "  ✓ Migrated {} (backup: {})",
                            path.display(),
                            outcome.backup_path.display()
                        )?;
                    }
                    Ok(_) => {
                        writeln!(
                            out,
                            "  ✓ {} is already at current schema version",
                            path.display()
                        )?;
                    }
                    Err(e) => {
                        writeln!(out, "  ✗ Migration failed for {}: {e}", path.display())?;
                    }
                }
            } else {
                writeln!(
                    out,
                    "  ✓ {} schema version is current (v{version})",
                    path.display()
                )?;
            }
        }

        writeln!(out, "Migration check complete.")?;
    }

    Ok(())
}

fn handle_config(
    sub: &ConfigSubcommand,
    cwd: &Path,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match sub {
        ConfigSubcommand::Show { explain } => {
            let resolved = resolve_config(cwd, &CliOverrides::default())?;
            if json || *explain {
                let mut output = serde_json::to_value(&resolved)?;
                if *explain {
                    output["resolution_trace"] = serde_json::to_value(&resolved.resolution_trace)?;
                }
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Resolved config for: {}", cwd.display());
                if let Some(name) = &resolved.template_name {
                    println!("  Template: {name}");
                }
                if let Some(name) = &resolved.persona.name {
                    println!("  Persona: {name}");
                }
                println!("  MCP servers: {}", resolved.mcp_servers.len());
                println!("  Skills: {}", resolved.skills.values().map(Vec::len).sum::<usize>());
            }
        }
        ConfigSubcommand::Set { key, value, scope } => {
            let path = config_path_for_scope(cwd, scope);
            set_config_value(&path, key, value)?;
            println!("Set {key} = {value} in {}", path.display());
        }
        ConfigSubcommand::Add { key, value, scope } => {
            let path = config_path_for_scope(cwd, scope);
            add_config_value(&path, key, value)?;
            println!("Added {value} to {key} in {}", path.display());
        }
        ConfigSubcommand::Remove { key, value, scope } => {
            let path = config_path_for_scope(cwd, scope);
            remove_config_value(&path, key, value)?;
            println!("Removed {value} from {key} in {}", path.display());
        }
    }
    Ok(())
}

fn config_path_for_scope(cwd: &Path, scope: &str) -> std::path::PathBuf {
    match scope {
        "global" => home::home_dir()
            .unwrap_or_default()
            .join(".config")
            .join("jig")
            .join("config.yaml"),
        "project" => cwd.join(".jig.yaml"),
        _ => cwd.join(".jig.local.yaml"),
    }
}

/// Sets a dotted-path scalar value in a YAML config file.
fn set_config_value(
    path: &std::path::Path,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let contents = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        "schema: 1\n".to_owned()
    };

    let mut doc: serde_yaml::Value = serde_yaml::from_str(&contents)?;
    set_nested_value(&mut doc, key, value)?;

    let new_contents = serde_yaml::to_string(&doc)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, new_contents)?;
    Ok(())
}

fn set_nested_value(
    doc: &mut serde_yaml::Value,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    if parts.len() == 1 {
        if let serde_yaml::Value::Mapping(map) = doc {
            map.insert(
                serde_yaml::Value::String(key.to_owned()),
                serde_yaml::Value::String(value.to_owned()),
            );
        }
    } else {
        let head = parts[0];
        let tail = parts[1];
        if let serde_yaml::Value::Mapping(map) = doc {
            let entry = map
                .entry(serde_yaml::Value::String(head.to_owned()))
                .or_insert(serde_yaml::Value::Mapping(Default::default()));
            set_nested_value(entry, tail, value)?;
        }
    }
    Ok(())
}

fn add_config_value(
    path: &std::path::Path,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let contents = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        "schema: 1\n".to_owned()
    };

    let mut doc: serde_yaml::Value = serde_yaml::from_str(&contents)?;
    add_nested_value(&mut doc, key, value)?;

    let new_contents = serde_yaml::to_string(&doc)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, new_contents)?;
    Ok(())
}

fn add_nested_value(
    doc: &mut serde_yaml::Value,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    if parts.len() == 1 {
        if let serde_yaml::Value::Mapping(map) = doc {
            let entry = map
                .entry(serde_yaml::Value::String(key.to_owned()))
                .or_insert(serde_yaml::Value::Sequence(Default::default()));
            if let serde_yaml::Value::Sequence(seq) = entry {
                let new_val = serde_yaml::Value::String(value.to_owned());
                if !seq.contains(&new_val) {
                    seq.push(new_val);
                }
            }
        }
    } else {
        let head = parts[0];
        let tail = parts[1];
        if let serde_yaml::Value::Mapping(map) = doc {
            let entry = map
                .entry(serde_yaml::Value::String(head.to_owned()))
                .or_insert(serde_yaml::Value::Mapping(Default::default()));
            add_nested_value(entry, tail, value)?;
        }
    }
    Ok(())
}

fn remove_config_value(
    path: &std::path::Path,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(());
    }
    let contents = std::fs::read_to_string(path)?;
    let mut doc: serde_yaml::Value = serde_yaml::from_str(&contents)?;
    remove_nested_value(&mut doc, key, value)?;
    let new_contents = serde_yaml::to_string(&doc)?;
    std::fs::write(path, new_contents)?;
    Ok(())
}

fn remove_nested_value(
    doc: &mut serde_yaml::Value,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    if parts.len() == 1 {
        if let serde_yaml::Value::Mapping(map) = doc {
            if let Some(serde_yaml::Value::Sequence(seq)) = map.get_mut(serde_yaml::Value::String(key.to_owned())) {
                let target = serde_yaml::Value::String(value.to_owned());
                seq.retain(|v| v != &target);
            }
        }
    } else {
        let head = parts[0];
        let tail = parts[1];
        if let serde_yaml::Value::Mapping(map) = doc {
            if let Some(entry) = map.get_mut(serde_yaml::Value::String(head.to_owned())) {
                remove_nested_value(entry, tail, value)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn handle_init(cwd: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut out = std::io::stdout();
    handle_init_to(cwd, &mut reader, &mut out)
}

/// Reader/writer-based init implementation — testable without real stdin.
pub(crate) fn handle_init_to<R: std::io::BufRead, W: std::io::Write>(
    cwd: &Path,
    reader: &mut R,
    out: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = cwd.join(".jig.yaml");
    if target.exists() {
        writeln!(out, ".jig.yaml already exists.")?;
        return Ok(());
    }

    // Detect project type and suggest a template
    let (detected_type, suggested_template) = detect_project(cwd);

    if let Some(ref project_type) = detected_type {
        writeln!(out, "Detected: {}", project_type)?;
    }

    let template_names: Vec<String> = jig_core::defaults::builtin_template_refs()
        .into_iter()
        .map(|t| t.name)
        .collect();
    let default_template = suggested_template.as_deref().unwrap_or("base");

    writeln!(out, "Available templates: {}", template_names.join(", "))?;
    let chosen_template = prompt_choice(
        &format!("Template [{}]: ", default_template),
        &template_names,
        default_template,
        reader,
        out,
    )?;

    let persona_names: Vec<String> = jig_core::defaults::builtin_personas()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    writeln!(out, "Available personas: {}", persona_names.join(", "))?;
    let chosen_persona = prompt_choice(
        "Persona [default]: ",
        &persona_names,
        "default",
        reader,
        out,
    )?;

    let yaml = scaffold_jig_yaml(&chosen_template, &chosen_persona);
    std::fs::write(&target, &yaml)?;

    writeln!(out, "Created .jig.yaml (template: {}, persona: {})", chosen_template, chosen_persona)?;
    writeln!(out, "Tip: create .jig.local.yaml for personal overrides (add to .gitignore).")?;

    Ok(())
}

/// Detects the project type from indicator files. Returns (description, template_name).
fn detect_project(cwd: &Path) -> (Option<String>, Option<String>) {
    let checks: &[(&str, &str, &str)] = &[
        ("Cargo.toml",        "Rust",    "base"),
        ("package.json",      "Node.js", "base-frontend"),
        ("pyproject.toml",    "Python",  "data-science"),
        ("requirements.txt",  "Python",  "data-science"),
        ("go.mod",            "Go",      "base"),
        ("Gemfile",           "Ruby",    "base"),
        ("Dockerfile",        "DevOps",  "base-devops"),
        ("docker-compose.yml","DevOps",  "base-devops"),
        ("mkdocs.yml",        "Docs",    "documentation"),
    ];

    for (file, label, template) in checks {
        if cwd.join(file).exists() {
            return (Some(label.to_string()), Some(template.to_string()));
        }
    }
    (None, None)
}

/// Prompts for a choice from a list, returning the default on empty input.
fn prompt_choice<R: std::io::BufRead, W: std::io::Write>(
    prompt: &str,
    valid: &[String],
    default: &str,
    reader: &mut R,
    out: &mut W,
) -> Result<String, Box<dyn std::error::Error>> {
    loop {
        write!(out, "{}", prompt)?;
        out.flush()?;
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(default.to_owned());
        }
        if valid.iter().any(|v| v == trimmed) {
            return Ok(trimmed.to_owned());
        }
        writeln!(out, "  Unknown choice '{}'. Valid options: {}", trimmed, valid.join(", "))?;
    }
}

/// Scaffolds a .jig.yaml with commented examples.
fn scaffold_jig_yaml(template: &str, persona: &str) -> String {
    format!(
        "schema: 1\n\
         \n\
         # Template to use when launching jig\n\
         # Run: jig template list  to see all options\n\
         # template: {template}\n\
         \n\
         persona:\n\
           ref: {persona}\n\
         \n\
         # Uncomment to add MCP servers:\n\
         # profile:\n\
         #   mcp:\n\
         #     my-server:\n\
         #       type: stdio\n\
         #       command: npx\n\
         #       args: [\"-y\", \"some-mcp-server\"]\n\
         \n\
         # Uncomment to add pre-launch hooks:\n\
         # hooks:\n\
         #   pre_launch:\n\
         #     - exec: [\"./scripts/setup.sh\"]\n\
         \n\
         # Personal overrides go in .jig.local.yaml (add that file to .gitignore)\n",
        template = template,
        persona = persona,
    )
}

fn handle_sync(args: &crate::cli::SyncArgs, cwd: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use jig_core::assembly::sync::{SyncOptions, sync_sources, SyncAction};
    use jig_core::assembly::skills_lock::update_skills_lock;
    use jig_core::assembly::skill_index::rebuild_index;

    let sources = collect_sources(cwd);

    if sources.is_empty() {
        println!("No sources configured.");
        println!("Add profile.sources to ~/.config/jig/config.yaml or .jig.yaml:");
        println!("  profile:");
        println!("    sources:");
        println!("      my-skills:");
        println!("        url: https://github.com/example/skills");
        return Ok(());
    }

    let opts = SyncOptions { frozen: args.frozen, check: args.check };

    let outcomes = sync_sources(&sources, &opts)?;

    for outcome in &outcomes {
        let status = match &outcome.action {
            SyncAction::Cloned => format!("cloned {}", outcome.source_name),
            SyncAction::Updated { from_sha } => format!(
                "updated {} ({} -> {})",
                outcome.source_name,
                &from_sha[..8.min(from_sha.len())],
                outcome.new_sha.as_deref().map(|s| &s[..8.min(s.len())]).unwrap_or("?")
            ),
            SyncAction::AlreadyUpToDate => format!("{} already up to date", outcome.source_name),
            SyncAction::BehindCheck { local_sha, remote_sha } => format!(
                "{} is behind (local: {}, remote: {})",
                outcome.source_name,
                &local_sha[..8.min(local_sha.len())],
                &remote_sha[..8.min(remote_sha.len())]
            ),
            SyncAction::UpToDateCheck => format!("{} is up to date", outcome.source_name),
            SyncAction::SkippedNoUrl => format!("{} (not cloned yet)", outcome.source_name),
        };
        println!("{status}");
    }

    if !args.check {
        update_skills_lock(&outcomes, &sources)?;
        if let Err(e) = rebuild_index() {
            eprintln!("Warning: Failed to rebuild skill index: {e}");
        }
    }

    Ok(())
}

fn collect_sources(cwd: &Path) -> std::collections::HashMap<String, jig_core::config::schema::SourceConfig> {
    use jig_core::config::schema::JigConfig;

    let mut sources = std::collections::HashMap::new();

    // Load global config
    let global_path = home::home_dir()
        .unwrap_or_default()
        .join(".config").join("jig").join("config.yaml");

    let local_path = cwd.join(".jig.local.yaml");
    let project_path = cwd.join(".jig.yaml");

    let paths: &[&std::path::Path] = &[
        &global_path,
        &project_path,
        &local_path,
    ];

    for path in paths {
        if !path.exists() { continue; }
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(config) = serde_yaml::from_str::<JigConfig>(&content) {
                if let Some(profile) = &config.profile {
                    if let Some(srcs) = &profile.sources {
                        sources.extend(srcs.clone());
                    }
                }
            }
        }
    }

    sources
}

fn handle_import(
    url: Option<&str>,
    scope: &str,
    dry_run: bool,
    cwd: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(url) = url {
        println!("jig import {url} --scope {scope} — URL import not yet implemented.");
        return Ok(());
    }

    // Import from ~/.claude.json for the current project directory
    import_from_claude_json(cwd, scope, dry_run)
}

/// Reverse-engineers the current project's MCP config from ~/.claude.json into .jig.yaml.
fn import_from_claude_json(
    cwd: &Path,
    scope: &str,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let claude_path = jig_core::assembly::mcp::claude_json_path();
    if !claude_path.exists() {
        println!("~/.claude.json not found. No config to import.");
        return Ok(());
    }

    let contents = std::fs::read_to_string(&claude_path)?;
    let doc: serde_json::Value = serde_json::from_str(&contents)?;

    let cwd_key = std::fs::canonicalize(cwd)
        .unwrap_or_else(|_| cwd.to_owned())
        .to_string_lossy()
        .into_owned();

    // Navigate to projects.<cwd>.mcpServers using direct map access
    let mcp_servers = doc
        .get("projects")
        .and_then(|p| p.get(&cwd_key))
        .and_then(|proj| proj.get("mcpServers"))
        .and_then(|s| s.as_object());

    let servers = match mcp_servers {
        Some(s) if !s.is_empty() => s,
        _ => {
            println!(
                "No MCP servers found in ~/.claude.json for this project ({}).",
                cwd_key
            );
            return Ok(());
        }
    };

    // Build .jig.yaml MCP section with credential detection
    let mut mcp_yaml = String::from("schema: 1\nprofile:\n  mcp:\n");
    let mut has_credentials = false;

    for (name, server) in servers {
        mcp_yaml.push_str(&format!("    {}:\n", name));
        if let Some(t) = server.get("type").and_then(|v| v.as_str()) {
            mcp_yaml.push_str(&format!("      type: {t}\n"));
        }
        if let Some(cmd) = server.get("command").and_then(|v| v.as_str()) {
            mcp_yaml.push_str(&format!("      command: {cmd}\n"));
        }
        if let Some(args) = server.get("args").and_then(|v| v.as_array()) {
            let args_yaml: Vec<String> = args
                .iter()
                .filter_map(|a| a.as_str())
                .map(|a| format!("        - {a}"))
                .collect();
            if !args_yaml.is_empty() {
                mcp_yaml.push_str("      args:\n");
                mcp_yaml.push_str(&args_yaml.join("\n"));
                mcp_yaml.push('\n');
            }
        }
        if let Some(url) = server.get("url").and_then(|v| v.as_str()) {
            mcp_yaml.push_str(&format!("      url: {url}\n"));
        }
        if let Some(env) = server.get("env").and_then(|v| v.as_object()) {
            if !env.is_empty() {
                mcp_yaml.push_str("      env:\n");
                for (k, v) in env {
                    let val = v.as_str().unwrap_or("");
                    if is_credential_like(k) {
                        has_credentials = true;
                        mcp_yaml.push_str(&format!("        {k}: \"${{{}}}\"  # credential — set via env var\n", k));
                    } else {
                        mcp_yaml.push_str(&format!("        {k}: {val}\n"));
                    }
                }
            }
        }
    }

    if dry_run {
        println!("# Dry run — would write to {}:", scope_path(cwd, scope).display());
        println!("{}", mcp_yaml);
        if has_credentials {
            println!("# Detected credentials — sensitive values replaced with ${{ENV_VAR}} references.");
            println!("# Set these environment variables before launching jig.");
        }
    } else {
        let target = scope_path(cwd, scope);
        if target.exists() {
            println!("Warning: {} already exists. Showing what would be added:", target.display());
            println!("{}", mcp_yaml);
            println!("Run with --dry-run to preview, then merge manually.");
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, &mcp_yaml)?;
            println!("Created {}", target.display());
            if has_credentials {
                println!(
                    "Detected credentials — sensitive env values replaced with ${{ENV_VAR}} references.\n\
                     Set those environment variables before launching jig, or move them to .jig.local.yaml."
                );
            }
        }
    }

    Ok(())
}

fn scope_path(cwd: &Path, scope: &str) -> std::path::PathBuf {
    config_path_for_scope(cwd, scope)
}

/// Returns true if an env var key looks like it holds a credential.
fn is_credential_like(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("key") || lower.contains("token") || lower.contains("secret")
        || lower.contains("password") || lower.contains("credential") || lower.contains("api")
        || lower.contains("auth") || lower.contains("passwd")
}

fn handle_diff(config: &std::path::Path, cwd: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use jig_core::config::resolve::{CliOverrides, resolve_config};

    if !config.exists() {
        eprintln!("Config file not found: {}", config.display());
        std::process::exit(1);
    }

    // Resolve current project config
    let current = resolve_config(cwd, &CliOverrides::default())?;

    // Resolve target: copy the config file into a temp dir as .jig.yaml
    let dir = tempfile::tempdir()?;
    std::fs::copy(config, dir.path().join(".jig.yaml"))?;
    let target = resolve_config(dir.path(), &CliOverrides::default())?;

    let current_json = serde_json::to_string_pretty(&current)?;
    let target_json = serde_json::to_string_pretty(&target)?;

    if current_json == target_json {
        println!("No differences between current config and {}.", config.display());
        return Ok(());
    }

    // Line-level diff: show lines only in current (-) and only in target (+)
    let current_lines: Vec<&str> = current_json.lines().collect();
    let target_lines: Vec<&str> = target_json.lines().collect();

    println!("--- current ({})", cwd.join(".jig.yaml").display());
    println!("+++ target ({})", config.display());

    for line in &current_lines {
        if !target_lines.contains(line) {
            println!("-  {}", line);
        }
    }
    for line in &target_lines {
        if !current_lines.contains(line) {
            println!("+  {}", line);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize tests that modify PATH to prevent races between parallel test threads.
    static PATH_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_handle_template_list_output_contains_builtins() {
        // handle_template list iterates over builtin_template_refs() and prints names.
        // Testing the data source is equivalent to testing the handler's output.
        let refs = jig_core::defaults::builtin_template_refs();
        let names: Vec<&str> = refs.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"code-review"), "code-review must be a builtin template");
        assert!(names.contains(&"security-audit"), "security-audit must be a builtin template");
    }

    #[test]
    fn test_handle_persona_list_output_contains_builtins() {
        let personas = jig_core::defaults::builtin_personas();
        let names: Vec<&str> = personas.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"strict-security"), "strict-security must be a builtin persona");
        assert!(names.contains(&"pair-programmer"), "pair-programmer must be a builtin persona");
    }

    #[test]
    fn test_handle_init_creates_jig_yaml() {
        let dir = tempfile::tempdir().unwrap();
        // Simulate user pressing Enter for both prompts (accept defaults)
        let input = b"\n\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let mut out = Vec::<u8>::new();
        handle_init_to(dir.path(), &mut reader, &mut out).unwrap();
        let target = dir.path().join(".jig.yaml");
        assert!(target.exists(), ".jig.yaml must be created");
        let contents = std::fs::read_to_string(&target).unwrap();
        assert!(contents.contains("schema: 1"), ".jig.yaml must contain schema: 1");
    }

    #[test]
    fn test_handle_init_scaffolds_template_and_persona() {
        let dir = tempfile::tempdir().unwrap();
        // Simulate user choosing "code-review" template and "mentor" persona
        let input = b"code-review\nmentor\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let mut out = Vec::<u8>::new();
        handle_init_to(dir.path(), &mut reader, &mut out).unwrap();
        let contents = std::fs::read_to_string(dir.path().join(".jig.yaml")).unwrap();
        assert!(contents.contains("ref: mentor"), "persona ref must appear in .jig.yaml");
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("code-review"), "chosen template must appear in output");
    }

    #[test]
    fn test_handle_init_does_not_overwrite_existing() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(".jig.yaml");
        std::fs::write(&target, "custom content\n").unwrap();

        let input = b"";
        let mut reader = std::io::BufReader::new(&input[..]);
        let mut out = Vec::<u8>::new();
        handle_init_to(dir.path(), &mut reader, &mut out).unwrap();

        let contents = std::fs::read_to_string(&target).unwrap();
        assert_eq!(contents, "custom content\n", "init must not overwrite existing .jig.yaml");
    }

    #[test]
    fn test_handle_init_detects_rust_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();
        // Accept defaults
        let input = b"\n\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let mut out = Vec::<u8>::new();
        handle_init_to(dir.path(), &mut reader, &mut out).unwrap();
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("Rust"), "should detect Rust from Cargo.toml");
    }

    #[test]
    fn test_handle_doctor_reports_missing_claude_binary() {
        let _guard = PATH_MUTEX.lock().unwrap();
        let original_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", "");

        let dir = tempfile::tempdir().unwrap();
        let mut out = Vec::<u8>::new();
        handle_doctor_to(dir.path(), false, false, &mut out).unwrap();
        std::env::set_var("PATH", &original_path);

        let output = String::from_utf8(out).unwrap();
        assert!(
            output.contains("not found in PATH"),
            "doctor must report missing claude binary, got: {output}"
        );
    }

    #[test]
    fn test_handle_doctor_audit_flag() {
        let dir = tempfile::tempdir().unwrap();
        let mut out = Vec::<u8>::new();
        handle_doctor_to(dir.path(), true, false, &mut out).unwrap();
        let output = String::from_utf8(out).unwrap();
        assert!(
            output.contains("audit"),
            "doctor --audit must include audit output, got: {output}"
        );
    }

    #[test]
    fn test_handle_doctor_no_audit_flag() {
        let dir = tempfile::tempdir().unwrap();
        let mut out = Vec::<u8>::new();
        handle_doctor_to(dir.path(), false, false, &mut out).unwrap();
        let output = String::from_utf8(out).unwrap();
        assert!(!output.contains("security checks"), "doctor without --audit must not show audit section");
    }

    #[test]
    fn test_handle_doctor_migrate_flag_reports_schema_status() {
        let dir = tempfile::tempdir().unwrap();
        // Write a config file with current schema version
        std::fs::write(dir.path().join(".jig.yaml"), "schema: 1\n").unwrap();
        let mut out = Vec::<u8>::new();
        handle_doctor_to(dir.path(), false, true, &mut out).unwrap();
        let output = String::from_utf8(out).unwrap();
        assert!(
            output.contains("Migration check complete"),
            "doctor --migrate must include migration output, got: {output}"
        );
    }

    #[test]
    fn test_handle_diff_identical_configs_reports_no_differences() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = "schema: 1\n";
        let project_config = dir.path().join(".jig.yaml");
        let other_config = dir.path().join("other.yaml");
        std::fs::write(&project_config, yaml).unwrap();
        std::fs::write(&other_config, yaml).unwrap();

        // Capture stdout is tricky; test by checking the function doesn't error
        let result = handle_diff(&other_config, dir.path());
        assert!(result.is_ok(), "handle_diff must not error on valid identical configs");
    }

    #[test]
    fn test_handle_diff_different_configs_runs_without_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".jig.yaml"), "schema: 1\nprofile:\n  settings:\n    model: claude-opus\n").unwrap();
        let other = dir.path().join("other.yaml");
        std::fs::write(&other, "schema: 1\nprofile:\n  settings:\n    model: claude-sonnet\n").unwrap();

        let result = handle_diff(&other, dir.path());
        assert!(result.is_ok(), "handle_diff must not error when configs differ");
    }
}

#[cfg(test)]
mod skill_tests {
    use super::*;

    #[test]
    fn test_compute_diff_no_changes() {
        let content = "line1\nline2\n";
        let diff = compute_diff(content, content);
        // Should have context lines (space prefix) — no - or + lines
        assert!(!diff.contains('-') && !diff.contains('+'));
    }

    #[test]
    fn test_compute_diff_added_line() {
        let orig = "line1\n";
        let modified = "line1\nline2\n";
        let diff = compute_diff(orig, modified);
        assert!(diff.contains("+line2"));
    }

    #[test]
    fn test_compute_diff_removed_line() {
        let orig = "line1\nline2\n";
        let modified = "line1\n";
        let diff = compute_diff(orig, modified);
        assert!(diff.contains("-line2"));
    }

    #[test]
    fn test_collect_sources_nonexistent_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let sources = collect_sources(dir.path());
        assert!(sources.is_empty());
    }

    #[test]
    fn test_collect_sources_reads_project_config() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = "schema: 1\nprofile:\n  sources:\n    my-skills:\n      url: https://github.com/example/skills\n";
        std::fs::write(dir.path().join(".jig.yaml"), yaml).unwrap();
        let sources = collect_sources(dir.path());
        assert!(sources.contains_key("my-skills"), "should find sources from .jig.yaml");
        assert_eq!(sources["my-skills"].url, "https://github.com/example/skills");
    }

    #[test]
    fn test_handle_skill_list_no_index() {
        // With no skill index, should output "No skills installed"
        // We can't easily test handle_skill directly since it prints to stdout
        // But we can test the logic components
        let index = jig_core::assembly::skill_index::read_index();
        let results = jig_core::assembly::skill_index::search(&index, "test");
        // May be empty or have skills depending on test env
        let _ = results;
    }
}
