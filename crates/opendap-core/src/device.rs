//! Device discovery.

use opendap_model::vm::DEFAULT_NAME;
use opendap_model::{Device, DeviceList, DeviceState, Mode, Transport, VmState, Warning};

use crate::ports::{VmBackend, VmProvider};

/// Every T1 on every transport this build supports (today: the virtual T1).
/// A transport that cannot be searched becomes a warning, not an error.
pub fn list<P: VmProvider>(provider: &P) -> DeviceList {
    let mut list = DeviceList::default();
    match provider.locate() {
        Ok(vm) => {
            let state = match vm.state() {
                VmState::Running => DeviceState::Online,
                VmState::Booting => DeviceState::Booting,
                VmState::Stopped => DeviceState::Offline,
            };
            let transport = Transport::Vm {
                name: DEFAULT_NAME.into(),
            };
            list.items.push(Device {
                id: transport.selector(),
                model: "T1".into(),
                is_virtual: true,
                transport,
                state,
                // The virtual T1 only boots into normal mode.
                mode: (state == DeviceState::Online).then_some(Mode::Normal),
                serial: None,
            });
        }
        Err(e) => list.warnings.push(Warning {
            code: e.code.code.into(),
            message: e.message,
        }),
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeProvider;

    #[test]
    fn the_virtual_t1_is_listed_with_its_state() {
        let l = list(&FakeProvider::running());
        assert_eq!(l.items.len(), 1);
        let d = &l.items[0];
        assert_eq!(
            (d.id.as_str(), d.state, d.mode, d.is_virtual),
            ("vm:default", DeviceState::Online, Some(Mode::Normal), true)
        );
        let l = list(&FakeProvider::booting());
        assert_eq!(
            (l.items[0].state, l.items[0].mode),
            (DeviceState::Booting, None)
        );
        let l = list(&FakeProvider::stopped());
        assert_eq!(l.items[0].state, DeviceState::Offline);
        assert!(l.warnings.is_empty());
    }

    #[test]
    fn a_missing_backend_is_a_warning() {
        let l = list(&FakeProvider::missing());
        assert!(l.items.is_empty());
        assert_eq!(l.warnings[0].code, "workspace.not_found");
    }
}
