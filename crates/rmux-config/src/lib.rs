#![forbid(unsafe_code)]
//! Configuration management for rmux.
//!
//! Loads and saves the rmux configuration from platform-appropriate
//! directories. Defines the config schema and provides import from
//! Ghostty config files.
//!
//! # Modules
//!
//! - `schema` — Configuration types and deserialization
//!
//! Will be fully implemented in Phase 1.

pub mod schema;
