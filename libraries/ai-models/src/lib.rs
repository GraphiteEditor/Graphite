//! # AI Model Registry & Metadata Schema
//!
//! This crate is the central nervous system for Graphite's AI capabilities.
//! It manages how the editor identifies, validates, and prepares to launch
//! various machine learning models through three logical layers:
//!
//! 1. **[`ModelManifest`]** – the serialisable "identity card" of a model.
//! 2. **[`License`]** – a safety gate that blocks non-permissive models.
//! 3. **[`ModelRegistry`]** – the centralised service that tracks every model's lifecycle.
pub mod manifest;
pub mod registry;

pub use manifest::{License, ModelManifest, TensorShape};
pub use registry::{ModelRegistry, ModelStatus, RegistryError};
