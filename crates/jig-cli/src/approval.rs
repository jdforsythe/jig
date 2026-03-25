use jig_core::security::approval::{ApprovalDecision, ApprovalRequest, ApprovalUi};

/// Terminal-based approval UI for headless/CLI mode.
pub struct TerminalApprovalUi {
    pub non_interactive: bool,
}

impl ApprovalUi for TerminalApprovalUi {
    fn prompt_approval(&self, req: &ApprovalRequest) -> ApprovalDecision {
        if self.non_interactive {
            // Non-interactive: auto-deny new approvals
            eprintln!(
                "Hook approval required but --non-interactive is set. Denying: {}",
                req.command
            );
            return ApprovalDecision::Denied;
        }

        let tier_label = match &req.tier {
            jig_core::config::schema::HookTrustTier::Full => "global config",
            jig_core::config::schema::HookTrustTier::Team => "team config",
            jig_core::config::schema::HookTrustTier::Personal => "personal config",
            jig_core::config::schema::HookTrustTier::ExternalSkill { url } => {
                eprintln!("Hook from external skill (source: {url}):");
                "external skill"
            }
        };

        // Show diff if command changed
        if let Some(prev) = &req.previous_command {
            eprintln!("Hook from {tier_label} has changed since last approval:");
            eprintln!("  - {prev}");
            eprintln!("  + {}", req.command);
        } else {
            eprintln!("Hook from {tier_label}: {}", req.command);
        }

        eprint!("Approve? [Y/n] ");
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() {
            return ApprovalDecision::Denied;
        }

        match input.trim().to_lowercase().as_str() {
            "" | "y" | "yes" => ApprovalDecision::Approved,
            _ => ApprovalDecision::Denied,
        }
    }
}
