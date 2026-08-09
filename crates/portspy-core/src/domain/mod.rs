//! PortSpy domain model.
//!
//! The domain is split into facts (what the operating system reports) and
//! inference (what PortSpy derives from those facts). The two never mix:
//!
//! * [`process::ProcessInfo`] holds facts.
//! * [`application::ApplicationInfo`] and [`origin::OriginInfo`] hold inference.

pub mod application;
pub mod confidence;
pub mod listener;
pub mod origin;
pub mod process;
pub mod protocol;
