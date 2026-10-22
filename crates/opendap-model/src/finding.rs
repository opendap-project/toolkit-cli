//! Findings, shared by `doctor` and other checks (`opendap.doctor/v1`).

use serde::{Deserialize, Serialize};

use crate::error::Fix;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warn,
    Error,
    Skip,
}

/// The result of one check. Every non-OK finding carries a fix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// Stable check ID, for example `vm.qemu`.
    pub id: String,
    pub status: Status,
    pub title: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub ok: u32,
    pub warn: u32,
    pub error: u32,
    pub skip: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub schema: String,
    /// False when any check is an error.
    pub ok: bool,
    pub summary: Summary,
    pub checks: Vec<Finding>,
}

impl DoctorReport {
    pub fn new(checks: Vec<Finding>) -> Self {
        let mut summary = Summary::default();
        for c in &checks {
            match c.status {
                Status::Ok => summary.ok += 1,
                Status::Warn => summary.warn += 1,
                Status::Error => summary.error += 1,
                Status::Skip => summary.skip += 1,
            }
        }
        DoctorReport {
            schema: "opendap.doctor/v1".into(),
            ok: summary.error == 0,
            summary,
            checks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(status: Status) -> Finding {
        Finding {
            id: "x.y".into(),
            status,
            title: "X".into(),
            message: String::new(),
            fix: None,
        }
    }

    #[test]
    fn report_summarises_and_fails_only_on_error() {
        let r = DoctorReport::new(vec![f(Status::Ok), f(Status::Warn), f(Status::Skip)]);
        assert!(r.ok);
        assert_eq!(
            r.summary,
            Summary {
                ok: 1,
                warn: 1,
                error: 0,
                skip: 1
            }
        );
        assert!(!DoctorReport::new(vec![f(Status::Error)]).ok);
    }
}
