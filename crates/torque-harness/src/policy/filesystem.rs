#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsAction {
    Read,
    Write,
    Edit,
    List,
    Glob,
    Grep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleEffect {
    Allow,
    Deny,
    RequireApproval,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemPermissionRule {
    pub effect: RuleEffect,
    pub action: FsAction,
    pub pattern: String,
}

impl FilesystemPermissionRule {
    pub fn new(effect: RuleEffect, action: FsAction, pattern: impl Into<String>) -> Self {
        Self {
            effect,
            action,
            pattern: pattern.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemDecision {
    Allow,
    Deny(String),
    RequireApproval(String),
}

pub fn evaluate_filesystem_rules(
    rules: &[FilesystemPermissionRule],
    action: FsAction,
    path: &str,
) -> FilesystemDecision {
    for rule in rules {
        if rule.action != action {
            continue;
        }
        if !matches_pattern(path, &rule.pattern) {
            continue;
        }
        return match rule.effect {
            RuleEffect::Allow => FilesystemDecision::Allow,
            RuleEffect::Deny => FilesystemDecision::Deny(format!(
                "filesystem policy denied {:?} on {}",
                action, path
            )),
            RuleEffect::RequireApproval => FilesystemDecision::RequireApproval(format!(
                "approval required for {:?} on {}",
                action, path
            )),
        };
    }

    FilesystemDecision::Deny(format!(
        "no filesystem rule matched {:?} on {}",
        action, path
    ))
}

fn matches_pattern(path: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        path == prefix || path.starts_with(&format!("{prefix}/"))
    } else {
        path == pattern
    }
}
