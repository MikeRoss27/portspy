//! PortSpy terminal interface.
//!
//! This crate renders state and translates keys. It never talks to the
//! operating system directly: it receives ready-made snapshots from the
//! collector and asks the injected [`portspy_core::platform::Platform`] for
//! destructive actions.

pub mod app;
pub mod collector;
pub mod format;
mod history;
pub mod input;
mod recent;
pub mod state;
pub mod ui;
