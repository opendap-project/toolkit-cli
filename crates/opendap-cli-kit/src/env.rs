//! Agent detection and interactivity rules (research §8.5, §8.10).

use std::io::IsTerminal;

/// Variables set by coding agents; `OPENDAP_AGENT=1|0` forces detection on or off.
pub const AGENT_VARS: &[&str] = &[
    "AI_AGENT",
    "CLAUDECODE",
    "CODEX_THREAD_ID",
    "CURSOR_AGENT",
    "GEMINI_CLI",
    "OPENCODE",
    "COPILOT_AGENT",
];

/// Whether an agent is driving the CLI, from an environment lookup.
pub fn agent_detected_with(get: impl Fn(&str) -> Option<String>) -> bool {
    match get("OPENDAP_AGENT").as_deref() {
        Some("1") | Some("true") => return true,
        Some("0") | Some("false") => return false,
        _ => {}
    }
    AGENT_VARS
        .iter()
        .any(|v| get(v).is_some_and(|s| !s.is_empty()))
}

pub fn agent_detected() -> bool {
    agent_detected_with(|k| std::env::var(k).ok())
}

/// Facts about the terminal the process runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tty {
    pub stdin: bool,
    pub stdout: bool,
    pub stderr: bool,
}

impl Tty {
    pub fn detect() -> Self {
        Tty {
            stdin: std::io::stdin().is_terminal(),
            stdout: std::io::stdout().is_terminal(),
            stderr: std::io::stderr().is_terminal(),
        }
    }
}

/// Prompts (and interactive sessions) are allowed only when stdin and stderr are
/// TTYs, no agent is detected, `CI` is unset and `--no-input` / `OPENDAP_NO_INPUT`
/// is not set.
pub fn interactive_allowed(no_input: bool, agent: bool, ci: bool, tty: Tty) -> bool {
    !no_input && !agent && !ci && tty.stdin && tty.stderr
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn detects_agents_and_honours_override() {
        assert!(!agent_detected_with(env(&[])));
        assert!(agent_detected_with(env(&[("CLAUDECODE", "1")])));
        assert!(!agent_detected_with(env(&[
            ("CLAUDECODE", "1"),
            ("OPENDAP_AGENT", "0")
        ])));
        assert!(agent_detected_with(env(&[("OPENDAP_AGENT", "1")])));
        assert!(!agent_detected_with(env(&[("CURSOR_AGENT", "")])));
    }

    #[test]
    fn interactivity_needs_every_condition() {
        let tty = Tty {
            stdin: true,
            stdout: true,
            stderr: true,
        };
        assert!(interactive_allowed(false, false, false, tty));
        assert!(!interactive_allowed(true, false, false, tty));
        assert!(!interactive_allowed(false, true, false, tty));
        assert!(!interactive_allowed(false, false, true, tty));
        assert!(!interactive_allowed(
            false,
            false,
            false,
            Tty {
                stdin: false,
                ..tty
            }
        ));
    }
}
