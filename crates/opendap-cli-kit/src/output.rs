//! Output modes and rendering (research §8.3). stdout carries data only.

use std::io::Write;

use serde::Serialize;

use crate::error::CliError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Tables and text for people. Free to change; scripts must not parse it.
    Human,
    /// One JSON document.
    Json,
    /// One JSON document per line (streams); single results are one line.
    Ndjson,
    /// Greppable text without decoration.
    Plain,
}

impl OutputMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "human" => Some(OutputMode::Human),
            "json" => Some(OutputMode::Json),
            "ndjson" => Some(OutputMode::Ndjson),
            "plain" => Some(OutputMode::Plain),
            _ => None,
        }
    }

    pub fn is_json(self) -> bool {
        matches!(self, OutputMode::Json | OutputMode::Ndjson)
    }

    /// Resolution order: `--json` / `--output` > `OPENDAP_OUTPUT` > JSON when an
    /// agent is detected > human when stdout is a TTY > JSON (spec §7.1).
    pub fn resolve(
        flag: Option<OutputMode>,
        json_flag: bool,
        env: Option<&str>,
        agent: bool,
        stdout_tty: bool,
    ) -> OutputMode {
        if json_flag {
            return OutputMode::Json;
        }
        if let Some(m) = flag {
            return m;
        }
        if let Some(m) = env.and_then(OutputMode::parse) {
            return m;
        }
        if agent || !stdout_tty {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }
}

/// Where a command writes its result.
#[derive(Debug, Clone, Copy)]
pub struct Output {
    pub mode: OutputMode,
    /// Pretty JSON only on a TTY when no agent is detected.
    pub pretty: bool,
}

impl Output {
    pub fn new(mode: OutputMode, agent: bool, stdout_tty: bool) -> Self {
        Output {
            mode,
            pretty: mode == OutputMode::Json && stdout_tty && !agent,
        }
    }

    pub fn json_string<T: Serialize>(&self, value: &T) -> String {
        let r = if self.pretty {
            serde_json::to_string_pretty(value)
        } else {
            serde_json::to_string(value)
        };
        r.expect("output documents serialize")
    }

    /// Print a success document in JSON modes, or the human text otherwise.
    pub fn emit<T: Serialize>(&self, value: &T, human: impl FnOnce() -> String) {
        let text = if self.mode.is_json() {
            self.json_string(value) + "\n"
        } else {
            human()
        };
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(text.as_bytes());
        let _ = out.flush();
    }

    /// Report an error: the envelope on stdout in JSON modes, text on stderr always.
    pub fn error(&self, err: &CliError) {
        if self.mode.is_json() {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{}", self.json_string(&err.document()));
        }
        eprint!("{}", crate::error::human(err));
    }
}

/// Status lines for people: stderr, never stdout.
pub fn note(msg: impl AsRef<str>) {
    eprintln!("{}", msg.as_ref());
}

/// Render rows as an aligned table.
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for r in rows {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let line = |cells: Vec<&str>| {
        let mut s = String::new();
        for (i, c) in cells.iter().enumerate() {
            if i + 1 == cells.len() {
                s.push_str(c);
            } else {
                s.push_str(&format!("{c:<w$}  ", w = widths[i]));
            }
        }
        s.trim_end().to_string() + "\n"
    };
    let mut s = line(headers.to_vec());
    for r in rows {
        s.push_str(&line(r.iter().map(String::as_str).collect()));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use OutputMode::*;

    #[test]
    fn resolution_order() {
        assert_eq!(
            OutputMode::resolve(Some(Human), true, None, false, true),
            Json
        );
        assert_eq!(
            OutputMode::resolve(Some(Plain), false, Some("json"), true, false),
            Plain
        );
        assert_eq!(
            OutputMode::resolve(None, false, Some("human"), true, false),
            Human
        );
        assert_eq!(
            OutputMode::resolve(None, false, Some("bogus"), false, true),
            Human
        );
        assert_eq!(OutputMode::resolve(None, false, None, true, true), Json);
        assert_eq!(OutputMode::resolve(None, false, None, false, true), Human);
        assert_eq!(OutputMode::resolve(None, false, None, false, false), Json);
    }

    #[test]
    fn json_is_compact_for_agents_and_pipes() {
        let v = serde_json::json!({"items": [], "schema": "x/v1"});
        assert_eq!(
            Output::new(Json, true, true).json_string(&v),
            r#"{"items":[],"schema":"x/v1"}"#
        );
        assert_eq!(
            Output::new(Json, false, false).json_string(&v),
            r#"{"items":[],"schema":"x/v1"}"#
        );
        assert!(
            Output::new(Json, false, true)
                .json_string(&v)
                .contains('\n')
        );
        assert!(
            !Output::new(Ndjson, false, true)
                .json_string(&v)
                .contains('\n')
        );
    }

    #[test]
    fn table_aligns_columns() {
        let t = table(
            &["ID", "STATE"],
            &[vec!["vm:default".into(), "online".into()]],
        );
        assert_eq!(t, "ID          STATE\nvm:default  online\n");
    }
}
