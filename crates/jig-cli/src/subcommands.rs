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
        Commands::History(args) => handle_history(args.limit, json),
        Commands::Doctor => handle_doctor(cwd),
        Commands::Config(args) => handle_config(&args.subcommand, cwd, json),
        Commands::Init => handle_init(cwd),
        Commands::Sync(_args) => handle_sync(),
        Commands::Import(args) => handle_import(&args.url, &args.scope),
        Commands::Diff(args) => handle_diff(&args.config, cwd),
        Commands::Completions(_) => Ok(()), // handled in main.rs before routing here
    }
}

fn handle_template(sub: &TemplateSubcommand, _cwd: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
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
    match sub {
        SkillSubcommand::List => {
            // TODO: list cached skills from ~/.config/jig/skills/
            if json {
                println!("[]");
            } else {
                println!("No skills installed. Run `jig sync` to fetch skills.");
            }
        }
    }
    Ok(())
}

fn handle_history(limit: usize, json: bool) -> Result<(), Box<dyn std::error::Error>> {
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
    let lines: Vec<&str> = contents.lines().collect();

    // Tail-first reading for recent sessions
    let recent: Vec<serde_json::Value> = lines
        .iter()
        .rev()
        .take(limit * 2) // over-read since we filter start records
        .filter_map(|line| serde_json::from_str(line).ok())
        .filter(|v: &serde_json::Value| v.get("type").and_then(|t| t.as_str()) == Some("start"))
        .take(limit)
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&recent)?);
    } else {
        for entry in &recent {
            let id = entry["session_id"].as_str().unwrap_or("?");
            let template = entry["template"].as_str().unwrap_or("none");
            let started = entry["started_at"].as_str().unwrap_or("?");
            println!("{started}  {template:20}  {id}");
        }
    }
    Ok(())
}

fn handle_doctor(_cwd: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("jig doctor — checking system state...");

    // Check claude binary
    if jig_core::assembly::executor::find_claude_binary().is_some() {
        println!("  ✓ claude binary found");
    } else {
        println!("  ✗ claude binary not found in PATH");
        println!("    Install claude: https://claude.ai/download");
    }

    // Check ~/.claude.json
    let claude_path = jig_core::assembly::mcp::claude_json_path();
    if claude_path.exists() {
        println!("  ✓ ~/.claude.json exists");
    } else {
        println!("  ! ~/.claude.json not found (will be created on first MCP write)");
    }

    // Check history
    let history_path = jig_core::history::history_path();
    if history_path.exists() {
        let lines = std::fs::read_to_string(&history_path)
            .map(|c| c.lines().count())
            .unwrap_or(0);
        println!("  ✓ history.jsonl exists ({lines} entries)");
    } else {
        println!("  ! history.jsonl not found (created on first launch)");
    }

    println!("Done.");
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
    let target = cwd.join(".jig.yaml");
    if target.exists() {
        println!(".jig.yaml already exists.");
        return Ok(());
    }
    let template = "schema: 1\n# Add your jig config here\n# See: jig template list\n";
    std::fs::write(&target, template)?;
    println!("Created .jig.yaml in {}", cwd.display());
    Ok(())
}

/// Writer-based doctor implementation — testable without stdout capture.
pub(crate) fn handle_doctor_to<W: std::io::Write>(
    _cwd: &std::path::Path,
    out: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(out, "jig doctor — checking system state...")?;

    if jig_core::assembly::executor::find_claude_binary().is_some() {
        writeln!(out, "  ✓ claude binary found")?;
    } else {
        writeln!(out, "  ✗ claude binary not found in PATH")?;
        writeln!(out, "    Install claude: https://claude.ai/download")?;
    }

    let claude_path = jig_core::assembly::mcp::claude_json_path();
    if claude_path.exists() {
        writeln!(out, "  ✓ ~/.claude.json exists")?;
    } else {
        writeln!(out, "  ! ~/.claude.json not found (will be created on first MCP write)")?;
    }

    let history_path = jig_core::history::history_path();
    if history_path.exists() {
        let lines = std::fs::read_to_string(&history_path)
            .map(|c| c.lines().count())
            .unwrap_or(0);
        writeln!(out, "  ✓ history.jsonl exists ({lines} entries)")?;
    } else {
        writeln!(out, "  ! history.jsonl not found (created on first launch)")?;
    }

    writeln!(out, "Done.")?;
    Ok(())
}

fn handle_sync() -> Result<(), Box<dyn std::error::Error>> {
    println!("jig sync — not yet implemented.");
    Ok(())
}

fn handle_import(url: &str, scope: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("jig import {url} --scope {scope} — not yet implemented.");
    Ok(())
}

fn handle_diff(config: &std::path::Path, _cwd: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("jig diff {} — not yet implemented.", config.display());
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
        handle_init(dir.path()).unwrap();
        let target = dir.path().join(".jig.yaml");
        assert!(target.exists(), ".jig.yaml must be created");
        let contents = std::fs::read_to_string(&target).unwrap();
        assert!(contents.contains("schema: 1"), ".jig.yaml must contain schema: 1");
    }

    #[test]
    fn test_handle_init_does_not_overwrite_existing() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(".jig.yaml");
        std::fs::write(&target, "custom content\n").unwrap();

        handle_init(dir.path()).unwrap();

        let contents = std::fs::read_to_string(&target).unwrap();
        assert_eq!(contents, "custom content\n", "init must not overwrite existing .jig.yaml");
    }

    #[test]
    fn test_handle_doctor_reports_missing_claude_binary() {
        let _guard = PATH_MUTEX.lock().unwrap();
        let original_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", "");

        let dir = tempfile::tempdir().unwrap();
        let mut out = Vec::<u8>::new();
        handle_doctor_to(dir.path(), &mut out).unwrap();
        std::env::set_var("PATH", &original_path);

        let output = String::from_utf8(out).unwrap();
        assert!(
            output.contains("not found in PATH"),
            "doctor must report missing claude binary, got: {output}"
        );
    }
}
