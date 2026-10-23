//! Application core of the OpenDAP toolkit: the use cases every surface
//! shares (CLI, OpenDAP Desktop, agents), written against [`ports`].
//!
//! No process spawning, no filesystem layout, no platform code: adapter crates
//! implement the ports, interfaces wire them in and render the results.

pub mod device;
pub mod doctor;
pub mod exec;
pub mod input;
pub mod ports;
pub mod power;
pub mod screen;
pub mod target;
pub mod vm;

#[cfg(any(test, feature = "contract"))]
pub mod contract;

#[cfg(test)]
mod fake;

pub use ports::{
    Captured, Clock, DeviceTransport, Exit, HostProbe, InputSink, ScreenSource, VmBackend,
    VmProvider,
};
pub use target::{Need, resolve_target};

/// The result of a use case that changes something: under dry-run, the plan
/// (one step per line, nothing changed); otherwise what it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome<T> {
    Plan(Vec<String>),
    Done(T),
}
