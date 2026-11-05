//! Builds the argv for `claude`. The program path is not part of the result.
use crate::model::InstanceConfig;

pub fn build_args(cfg: &InstanceConfig) -> Vec<String> {
    let mut args = vec!["remote-control".to_owned()];
    if let Some(name) = &cfg.name {
        args.extend(["--name".to_owned(), name.clone()]);
    }
    args.push(
        if cfg.chrome {
            "--chrome"
        } else {
            "--no-chrome"
        }
        .to_owned(),
    );
    if let Some(mode) = &cfg.permission_mode {
        args.extend(["--permission-mode".to_owned(), mode.clone()]);
    }
    if let Some(spawn) = &cfg.spawn {
        args.extend(["--spawn".to_owned(), spawn.clone()]);
    }
    if let Some(capacity) = cfg.capacity {
        args.extend(["--capacity".to_owned(), capacity.to_string()]);
    }
    match cfg.create_session_in_dir {
        Some(true) => args.push("--create-session-in-dir".to_owned()),
        Some(false) => args.push("--no-create-session-in-dir".to_owned()),
        None => {}
    }
    if let Some(prefix) = &cfg.session_name_prefix {
        args.extend([
            "--remote-control-session-name-prefix".to_owned(),
            prefix.clone(),
        ]);
    }
    args.extend(cfg.extra_args.iter().cloned());
    args
}

/// True when a start may add `--continue` to reattach to the last session of the folder.
/// Not when the user set the session in `extra_args`, and not in single-session spawn mode,
/// where the session ending is the reason for the exit.
pub fn can_continue(cfg: &InstanceConfig) -> bool {
    let set_by_user = cfg
        .extra_args
        .iter()
        .any(|a| a == "-c" || a == "--continue" || a.starts_with("--session-id"));
    !set_by_user && cfg.spawn.as_deref() != Some("session")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_instance_has_only_required_flags() {
        let cfg = InstanceConfig::default();
        assert_eq!(build_args(&cfg), ["remote-control", "--no-chrome"]);
    }

    #[test]
    fn every_option_is_mapped_in_order() {
        let cfg = InstanceConfig {
            name: Some("box".into()),
            chrome: true,
            permission_mode: Some("acceptEdits".into()),
            spawn: Some("worktree".into()),
            capacity: Some(4),
            create_session_in_dir: Some(false),
            session_name_prefix: Some("rc".into()),
            extra_args: vec!["--verbose".into()],
            ..Default::default()
        };
        assert_eq!(
            build_args(&cfg),
            [
                "remote-control",
                "--name",
                "box",
                "--chrome",
                "--permission-mode",
                "acceptEdits",
                "--spawn",
                "worktree",
                "--capacity",
                "4",
                "--no-create-session-in-dir",
                "--remote-control-session-name-prefix",
                "rc",
                "--verbose",
            ]
        );
    }

    #[test]
    fn create_session_flag_is_positive_when_true() {
        let cfg = InstanceConfig {
            create_session_in_dir: Some(true),
            ..Default::default()
        };
        assert!(build_args(&cfg).contains(&"--create-session-in-dir".to_owned()));
    }

    #[test]
    fn continue_is_skipped_when_user_picks_the_session() {
        assert!(can_continue(&InstanceConfig::default()));
        for extra in ["-c", "--continue", "--session-id=abc", "--session-id"] {
            let cfg = InstanceConfig {
                extra_args: vec![extra.into()],
                ..Default::default()
            };
            assert!(!can_continue(&cfg), "{extra}");
        }
        let single = InstanceConfig {
            spawn: Some("session".into()),
            ..Default::default()
        };
        assert!(!can_continue(&single));
    }
}
