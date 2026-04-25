use torque_harness::policy::filesystem::{
    evaluate_filesystem_rules, FilesystemDecision, FilesystemPermissionRule, FsAction, RuleEffect,
};

fn default_rules() -> Vec<FilesystemPermissionRule> {
    vec![
        FilesystemPermissionRule::new(RuleEffect::Allow, FsAction::Read, "/workspace/**"),
        FilesystemPermissionRule::new(RuleEffect::Allow, FsAction::List, "/workspace/**"),
        FilesystemPermissionRule::new(RuleEffect::Allow, FsAction::Glob, "/workspace/**"),
        FilesystemPermissionRule::new(RuleEffect::Allow, FsAction::Grep, "/workspace/**"),
        FilesystemPermissionRule::new(RuleEffect::Allow, FsAction::Write, "/scratch/**"),
        FilesystemPermissionRule::new(RuleEffect::Allow, FsAction::Edit, "/scratch/**"),
        FilesystemPermissionRule::new(RuleEffect::Deny, FsAction::Write, "/workspace/.git/**"),
        FilesystemPermissionRule::new(RuleEffect::Deny, FsAction::Edit, "/workspace/.git/**"),
    ]
}

#[test]
fn filesystem_permissions_tests_allows_workspace_read() {
    let decision = evaluate_filesystem_rules(
        &default_rules(),
        FsAction::Read,
        "/workspace/README.md",
    );
    assert_eq!(decision, FilesystemDecision::Allow);
}

#[test]
fn filesystem_permissions_tests_denies_git_write() {
    let decision = evaluate_filesystem_rules(
        &default_rules(),
        FsAction::Write,
        "/workspace/.git/config",
    );
    assert!(matches!(decision, FilesystemDecision::Deny(_)));
}

#[test]
fn filesystem_permissions_tests_allows_scratch_write() {
    let decision = evaluate_filesystem_rules(
        &default_rules(),
        FsAction::Write,
        "/scratch/notes.txt",
    );
    assert_eq!(decision, FilesystemDecision::Allow);
}

#[test]
fn filesystem_permissions_tests_defaults_to_deny() {
    let decision = evaluate_filesystem_rules(
        &default_rules(),
        FsAction::Write,
        "/unknown/path.txt",
    );
    assert!(matches!(decision, FilesystemDecision::Deny(_)));
}
