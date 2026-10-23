//! Shared domain model of the OpenDAP toolkit: the innermost layer. No I/O,
//! no platform code.
//!
//! Plain serde types with versioned schema IDs (`opendap.<kind>/v1`). Fields
//! are only added within a `/v1`; removing or renaming one means `/v2`.

pub mod device;
pub mod error;
pub mod exec;
pub mod finding;
pub mod input;
pub mod screen;
pub mod vm;

pub use device::{Device, DeviceList, DeviceState, Mode, Target, Transport, Warning};
pub use error::{Error, ErrorBody, ErrorCode, ErrorDocument, Fix};
pub use exec::ExecResult;
pub use finding::{DoctorReport, Finding, Status, Summary};
pub use input::{InputEvent, InputSent, Role};
pub use screen::ScreenCapture;
pub use vm::{Endpoints, ImageInfo, VmInfo, VmState};

/// Schema IDs published by this model version.
pub const SCHEMAS: &[&str] = &[
    "opendap.device/v1",
    "opendap.device_list/v1",
    "opendap.doctor/v1",
    "opendap.error/v1",
    "opendap.exec/v1",
    "opendap.input/v1",
    "opendap.schema/v1",
    "opendap.screen_capture/v1",
    "opendap.version/v1",
    "opendap.vm/v1",
    "opendap.vm_list/v1",
];
