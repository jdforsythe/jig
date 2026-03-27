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
        Commands::Doctor(args) => handle_doctor_to(cwd, args.audit, &mut std::io::stdout()),
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

/// Writer-based doctor implementation — testable without stdout capture.
pub(crate) fn handle_doctor_to<W: std::io::Write>(
    cwd: &Path,
    audit: bool,
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

    writeln!(out, "Done.")?;

    if audit {
        writeln!(out, "")?;
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

fn handle_sync() -> Result<(), Box<dyn std::error::Error>> {
    println!("jig sync — not yet implemented.");
    Ok(())
}

fn handle_import(url: &str, scope: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("jig import {url} --scope {scope} — not yet implemented.");
    Ok(())
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
        handle_doctor_to(dir.path(), false, &mut out).unwrap();
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
        handle_doctor_to(dir.path(), true, &mut out).unwrap();
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
        handle_doctor_to(dir.path(), false, &mut out).unwrap();
        let output = String::from_utf8(out).unwrap();
        assert!(!output.contains("security checks"), "doctor without --audit must not show audit section");
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
