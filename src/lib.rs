//! # mochapot_lib
//!
//! Experimental utilities and concurrency primitives. (that only exists because i suffer chronically from "not invented here syndrome")
//!
//! ## ⚠️ Warning
//!
//! This crate is **not production-ready**.
//!
//! - APIs may change without notice
//! - Some components are experimental
//! - Concurrency primitives are not formally verified
//!
//! Use at your own risk.
//!
//! ## Overview
//!
//! `mochapot_lib` provides:
//!
//! - A flexible circular container (`cycler`)
//! - Experimental concurrency tools (`concurrency`, feature-gated)
//!
//! The crate is designed as both a reusable toolbox and a space for
//! exploring low-level Rust patterns.
//!
//! ## Modules
//!
//! ### `cycler`
//! Circular data structures for iterating over values.
//!
//! ### `concurrency` *(feature-gated)*
//! Experimental synchronization primitives.
//!
//! ## Example
//!
//! ```rust
//! use mochapot_lib::cycler::MochaCycler;
//!
//! let mut cycler = MochaCycler::new(vec![1, 2, 3]).unwrap();
//! cycler.advance_then_get(1);
//! assert_eq!(cycler.get_current(), 2);
//! ```
//!
//! ## Philosophy
//!
//! - Bad decisions
//! - Good luck
//! - Low-level control when needed
//! - Experimentation over strict guarantees

#[cfg(feature = "concurrency")]
pub mod concurrency;
pub mod cycler;
mod helper_functions;