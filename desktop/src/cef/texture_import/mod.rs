//! Unified texture import system for CEF hardware acceleration
//!
//! This module provides a platform-agnostic interface for importing shared textures
//! from CEF into wgpu, with automatic fallback to CPU textures when hardware
//! acceleration is not available.
//!
//! # Supported Platforms
//!
//! - **Linux**: DMA-BUF via Vulkan external memory
//! - **Windows**: D3D11 shared textures via Vulkan interop
//! - **macOS**: IOSurface via Metal/Vulkan interop
//!
//! # Usage
//!
//! ```rust
//! use crate::cef::texture_import;
//!
//! // Import texture with automatic platform detection
//! let texture = texture_import::import_texture(shared_handle, &device)?;
//! ```
//!
//! # Features
//!
//! - `accelerated_paint` - Base feature for texture import
//! - `accelerated_paint_dmabuf` - Linux DMA-BUF support
//! - `accelerated_paint_d3d11` - Windows D3D11 support
//! - `accelerated_paint_iosurface` - macOS IOSurface support

pub mod common;

#[cfg(target_os = "linux")]
pub mod dmabuf;

#[cfg(target_os = "windows")]
pub mod d3d11;

#[cfg(target_os = "macos")]
pub mod iosurface;

// Re-export commonly used types
pub use common::import_texture;
