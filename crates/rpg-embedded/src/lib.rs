//! Shared frontend primitives for embedded-graphics-based runtimes.
//!
//! This crate contains the common gameplay map-view model used by both the
//! hardware `tdeck` app and the host-side `sixel-console` launcher.

#![no_std]

extern crate alloc;

pub mod app;
pub mod info_overlay;
pub mod input;
pub mod list;
pub mod map_view;
pub mod render;
pub mod save_overlay;
pub mod session;
pub mod splash;
