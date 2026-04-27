//! HTTP integration tests for the Solid Community Server.
//!
//! This crate is a **library** consumed by the `solid-test` binary in
//! `crates/cli`.  It is deliberately decoupled from `#[tokio::test]` so that
//! the same suites can be driven against any running server — the original
//! TypeScript CSS or this Rust port.
//!
//! # Architecture
//!
//! ```text
//! TestSuite
//!   └─ Vec<Suite>               (one per logical area, e.g. "resource-crud")
//!        └─ Vec<Case>           (one per individual scenario)
//!             └─ async fn(ctx)  (executes HTTP requests, asserts responses)
//! ```
//!
//! A [`TestSuite`] is built from a [`RunConfig`] and driven by calling
//! [`TestSuite::run`].  The runner prints a TAP-like summary to stdout and
//! returns `true` iff every case passed.

pub mod client;
pub mod runner;
pub mod suites;

pub use runner::{RunConfig, TestSuite};
