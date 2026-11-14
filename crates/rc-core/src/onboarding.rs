//! claude's one-time questions and login problems that keep `remote-control` from starting.
//! rc-keeper never answers these itself. It only detects them and explains them in `last_error`.

/// Text of claude's one-time consent question. A person must answer it in a terminal.
pub const ENABLE_PROMPT: &str = "Enable Remote Control?";
/// Printed when the login token lacks the scope that remote sessions need.
pub const SCOPE_MARKER: &str = "does not meet scope requirement user:sessions:claude_code";

pub const ENABLE_MESSAGE: &str = "rc asks a one-time \"Enable Remote Control?\" question. \
    Run `claude remote-control` in a terminal once and answer y.";
pub const SCOPE_MESSAGE: &str = "claude's login lacks the remote sessions scope. \
    Run `claude auth login` (or /login in claude) again, then restart the instance.";

/// The explanation for a line that reports the missing remote sessions scope.
pub fn scope_problem(line: &str) -> Option<&'static str> {
    line.contains(SCOPE_MARKER).then_some(SCOPE_MESSAGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_line_is_explained() {
        let line = "Error: Registration: Access denied (403): OAuth token does not meet scope requirement user:sessions:claude_code. Check your organization permissions.";
        assert_eq!(scope_problem(line), Some(SCOPE_MESSAGE));
    }

    #[test]
    fn other_errors_are_not_scope_problems() {
        assert_eq!(scope_problem("Error: bad flag --foo"), None);
    }

    #[test]
    fn messages_have_no_stray_continuation_spaces() {
        assert!(!ENABLE_MESSAGE.contains("  "));
        assert!(!SCOPE_MESSAGE.contains("  "));
    }
}
