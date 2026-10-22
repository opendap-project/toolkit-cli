//! The error every command returns, rendered as the `opendap.error/v1`
//! envelope on stdout (JSON modes) and as text on stderr (always).

use opendap_model::error::{ErrorBody, ErrorCode, ErrorDocument, Fix};

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct CliError {
    pub code: &'static ErrorCode,
    pub message: String,
    pub hint: Option<String>,
    pub fix: Vec<Fix>,
    pub details: Option<serde_json::Value>,
}

impl CliError {
    pub fn new(code: &'static ErrorCode, message: impl Into<String>) -> Self {
        CliError {
            code,
            message: message.into(),
            hint: None,
            fix: Vec::new(),
            details: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn fix(mut self, command: impl Into<String>) -> Self {
        self.fix.push(Fix::command(command));
        self
    }

    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn exit_code(&self) -> u8 {
        self.code.exit
    }

    pub fn body(&self) -> ErrorBody {
        ErrorBody {
            code: self.code.code.to_string(),
            message: self.message.clone(),
            hint: self
                .hint
                .clone()
                .unwrap_or_else(|| self.code.hint.to_string()),
            fix: self.fix.clone(),
            retryable: self.code.retryable,
            details: self.details.clone(),
            docs: format!("opendap help errors {}", self.code.code),
        }
    }

    pub fn document(&self) -> ErrorDocument {
        ErrorDocument::new(self.body())
    }

    /// Human rendering for stderr.
    pub fn human(&self) -> String {
        let b = self.body();
        let mut s = format!("error[{}]: {}\n  hint: {}\n", b.code, b.message, b.hint);
        for f in &b.fix {
            if let Some(c) = &f.command {
                s.push_str(&format!("  fix: {c}\n"));
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendap_model::error as codes;

    #[test]
    fn envelope_has_the_documented_shape() {
        let e = CliError::new(&codes::VM_NOT_RUNNING, "vm:default is stopped")
            .fix("opendap vm start")
            .details(serde_json::json!({"target": "vm:default"}));
        let v = serde_json::to_value(e.document()).unwrap();
        assert_eq!(v["schema"], "opendap.error/v1");
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "vm.not_running");
        assert_eq!(v["error"]["message"], "vm:default is stopped");
        assert_eq!(v["error"]["hint"], codes::VM_NOT_RUNNING.hint);
        assert_eq!(
            v["error"]["fix"],
            serde_json::json!([{"command": "opendap vm start"}])
        );
        assert_eq!(v["error"]["retryable"], false);
        assert_eq!(v["error"]["details"]["target"], "vm:default");
        assert_eq!(v["error"]["docs"], "opendap help errors vm.not_running");
        assert_eq!(e.exit_code(), 10);
    }

    #[test]
    fn explicit_hint_wins_and_details_are_optional() {
        let e = CliError::new(&codes::INPUT_INVALID, "bad").hint("try this");
        let v = serde_json::to_value(e.document()).unwrap();
        assert_eq!(v["error"]["hint"], "try this");
        assert!(v["error"].get("details").is_none());
        assert_eq!(e.exit_code(), 2);
    }
}
