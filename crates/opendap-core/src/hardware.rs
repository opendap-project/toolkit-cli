//! The physical side of a virtual device: reading it, changing it by hand, and the state of a
//! switch that makes such a change, for surfaces that show one (the simulator window, Desktop).

use opendap_model::error as codes;
use opendap_model::{Attachment, Error, HardwareChange, HardwareChanged, HardwareState, Transport};

use crate::Outcome;
use crate::ports::HardwareBench;

/// The hardware's state.
pub fn state<B: HardwareBench>(bench: &B, target: &Transport) -> Result<HardwareState, Error> {
    bench.hardware(target)
}

/// Make one change, unless the hardware already is so (`changed: false`): attaching what is
/// attached does nothing, as with a real card already in its socket.
pub fn change<B: HardwareBench>(
    bench: &B,
    target: Transport,
    change: HardwareChange,
    dry_run: bool,
) -> Result<Outcome<HardwareChanged>, Error> {
    if let HardwareChange::Battery { percent } = change
        && percent > 100
    {
        return Err(Error::new(
            &codes::INPUT_INVALID,
            format!("battery {percent} %: the charge is 0 to 100"),
        ));
    }
    if let HardwareChange::Light { lux } = change
        && lux > opendap_model::LIGHT_MAX_LUX
    {
        return Err(Error::new(
            &codes::INPUT_INVALID,
            format!("light {lux} lx: 0 to {} lx", opendap_model::LIGHT_MAX_LUX),
        ));
    }
    let before = bench.hardware(&target)?;
    let already = match change {
        HardwareChange::Attach { attachment } => before.has(attachment),
        HardwareChange::Detach { attachment } => !before.has(attachment),
        HardwareChange::Battery { percent } => before.battery == Some(percent),
        HardwareChange::Light { lux } => before.light == Some(lux),
    };
    if dry_run {
        let plan = if already {
            vec![]
        } else {
            vec![bench.describe(&change)]
        };
        return Ok(Outcome::Plan(plan));
    }
    if already {
        return Ok(Outcome::Done(HardwareChanged {
            state: before,
            change,
            changed: false,
        }));
    }
    bench.apply(&change)?;
    let state = bench.hardware(&target)?;
    Ok(Outcome::Done(HardwareChanged {
        state,
        change,
        changed: true,
    }))
}

/// A switch that attaches or detaches one thing. A change in flight locks it until the hardware
/// reports the change done, so nothing is sent twice and nothing is undone half-way (a USB disk
/// is not unplugged while it is still being plugged in).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Switch {
    #[default]
    Off,
    Attaching,
    On,
    Detaching,
}

impl Switch {
    /// In flight: the switch takes no request.
    pub fn busy(self) -> bool {
        matches!(self, Switch::Attaching | Switch::Detaching)
    }

    pub fn on(self) -> bool {
        matches!(self, Switch::On | Switch::Detaching)
    }

    /// A person asks for `on`. The change to make, if any: none while in flight or already so.
    pub fn request(&mut self, on: bool) -> Option<HardwareChangeKind> {
        if self.busy() || self.on() == on {
            return None;
        }
        *self = if on {
            Switch::Attaching
        } else {
            Switch::Detaching
        };
        Some(if on {
            HardwareChangeKind::Attach
        } else {
            HardwareChangeKind::Detach
        })
    }

    /// The hardware reports `attached`. A change in flight ends only when the hardware has its
    /// end; a settled switch follows the hardware (unplugged from elsewhere).
    pub fn observe(&mut self, attached: bool) {
        *self = match (*self, attached) {
            (Switch::Attaching, false) | (Switch::Detaching, true) => *self,
            (_, true) => Switch::On,
            (_, false) => Switch::Off,
        };
    }

    /// The change failed: back to what the hardware has.
    pub fn failed(&mut self, attached: bool) {
        *self = if attached { Switch::On } else { Switch::Off };
    }
}

/// Which way a [`Switch`] request goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareChangeKind {
    Attach,
    Detach,
}

impl HardwareChangeKind {
    pub fn of(self, attachment: Attachment) -> HardwareChange {
        match self {
            HardwareChangeKind::Attach => HardwareChange::Attach { attachment },
            HardwareChangeKind::Detach => HardwareChange::Detach { attachment },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract;
    use crate::fake::FakeBench;

    fn vm() -> Transport {
        Transport::Vm {
            name: "default".into(),
        }
    }

    #[test]
    fn attaches_once() {
        let bench = FakeBench::default();
        let card = HardwareChange::Attach {
            attachment: Attachment::Microsd,
        };
        let Outcome::Done(first) = change(&bench, vm(), card, false).unwrap() else {
            panic!()
        };
        assert!(first.changed && first.state.has(Attachment::Microsd));
        let Outcome::Done(again) = change(&bench, vm(), card, false).unwrap() else {
            panic!()
        };
        assert!(!again.changed);
        assert_eq!(bench.applied.borrow().len(), 1);
        assert_eq!(
            serde_json::to_value(&first).unwrap()["change"],
            serde_json::json!("attach")
        );
    }

    #[test]
    fn plans_only_what_changes() {
        let bench = FakeBench::default();
        let out = HardwareChange::Detach {
            attachment: Attachment::UsbAudio,
        };
        assert!(
            matches!(change(&bench, vm(), out, true).unwrap(), Outcome::Plan(p) if p.is_empty())
        );
        let charge = HardwareChange::Battery { percent: 40 };
        assert!(
            matches!(change(&bench, vm(), charge, true).unwrap(), Outcome::Plan(p) if p.len() == 1)
        );
        assert!(bench.applied.borrow().is_empty());
        let over = HardwareChange::Battery { percent: 101 };
        assert_eq!(
            change(&bench, vm(), over, false).unwrap_err().code.code,
            "input.invalid"
        );
    }

    #[test]
    fn a_switch_in_flight_takes_no_request() {
        let mut s = Switch::Off;
        assert_eq!(s.request(true), Some(HardwareChangeKind::Attach));
        assert_eq!(s, Switch::Attaching);
        // Pressed again, or turned back, while it attaches: nothing.
        assert_eq!(s.request(true), None);
        assert_eq!(s.request(false), None);
        s.observe(false);
        assert_eq!(s, Switch::Attaching, "ends only when the hardware has it");
        s.observe(true);
        assert_eq!(s, Switch::On);
        assert_eq!(s.request(true), None, "already on");
        assert_eq!(s.request(false), Some(HardwareChangeKind::Detach));
        s.failed(true);
        assert_eq!(s, Switch::On);
        s.observe(false);
        assert_eq!(s, Switch::Off, "a settled switch follows the hardware");
    }

    #[test]
    fn the_fake_passes_the_contract() {
        contract::hardware_bench(&FakeBench::default());
    }
}
