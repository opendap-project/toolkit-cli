//! Shared domain model of the OpenDAP toolkit.
//!
//! Plain serde types with versioned schema IDs (`opendap.<kind>/v1`). Fields
//! are only added within a `/v1`; removing or renaming one means `/v2`.

pub mod device;
pub mod error;
pub mod finding;

pub use device::{Device, DeviceState, Mode, Target, Transport};
pub use error::{ErrorBody, ErrorCode, ErrorDocument, Fix};
pub use finding::{DoctorReport, Finding, Status, Summary};

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
