//! Experimental concurrency primitives.
//!
//! ## ⚠️ Warning
//!
//! This module is **experimental** and may contain (certainly does) unsafe or (and)
//! unverified behavior.
//!
//! ## Overview
//!
//! Includes custom synchronization tools such as:
//!
//! - [`MochaLock`] – experimental lock implementation
//!
//! These primitives explore:
//! - Reader/writer coordination
//! - Atomic state handling
//! - Custom blocking behavior
//!
//! ## Feature Flag
//!
//! This module is only available with the `concurrency` feature enabled.

pub mod mocha_lock;

#[cfg(test)]
mod tests;