//! `doctor`: host checks, each with a status and a fix.

use opendap_model::DoctorReport;

use crate::ports::HostProbe;

/// Run every probe in order and summarize.
pub fn run(probes: &[&dyn HostProbe]) -> DoctorReport {
    DoctorReport::new(probes.iter().flat_map(|p| p.check()).collect())
}

#[cfg(test)]
mod tests {
    use opendap_model::Status;

    use super::*;
    use crate::fake::FakeProbe;

    #[test]
    fn findings_keep_probe_order_and_errors_fail_the_report() {
        let a = FakeProbe::one("a.ok", Status::Ok);
        let b = FakeProbe::one("b.warn", Status::Warn);
        let r = run(&[&a, &b]);
        assert!(r.ok);
        let ids: Vec<_> = r.checks.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["a.ok", "b.warn"]);
        let c = FakeProbe::one("c.err", Status::Error);
        assert!(!run(&[&a, &c]).ok);
    }
}
