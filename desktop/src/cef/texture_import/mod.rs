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
//! - **macOS**: IOSurface via Metal native API
//!
//! # Features
//!
//! - `accelerated_paint` - Base feature for texture import
//! - `accelerated_paint_dmabuf` - Linux DMA-BUF support
//! - `accelerated_paint_d3d11` - Windows D3D11 support
//! - `accelerated_paint_iosurface` - macOS IOSurface support

pub(crate) mod common;

pub(crate) mod shared_texture_handle;
pub(crate) use shared_texture_handle::SharedTextureHandle;

#[cfg(target_os = "linux")]
pub(crate) mod dmabuf;

#[cfg(target_os = "windows")]
pub(crate) mod d3d11;

#[cfg(target_os = "macos")]
pub(crate) mod iosurface;

/// Result type for texture import operations
pub type TextureImportResult = Result<wgpu::Texture, TextureImportError>;

/// Errors that can occur during texture import
#[derive(Debug, thiserror::Error)]
pub enum TextureImportError {
	#[error("Invalid texture handle: {0}")]
	InvalidHandle(String),

	#[error("Unsupported texture format: {format:?}")]
	UnsupportedFormat { format: cef::sys::cef_color_type_t },

	#[error("Hardware acceleration not available: {reason}")]
	HardwareUnavailable { reason: String },

	#[error("Vulkan operation failed: {operation}")]
	#[cfg(not(target_os = "macos"))]
	VulkanError { operation: String },

	#[error("Platform-specific error: {message}")]
	PlatformError { message: String },

	#[error("Unsupported platform for texture import")]
	UnsupportedPlatform,
}

/// Trait for platform-specific texture importers
pub trait TextureImporter {
	fn new(info: &cef::AcceleratedPaintInfo) -> Self;

	/// Import the texture into wgpu, with automatic fallback to CPU texture
	fn import_to_wgpu(&self, device: &wgpu::Device) -> TextureImportResult;

	/// Check if hardware acceleration is available for this texture
	fn supports_hardware_acceleration(&self, device: &wgpu::Device) -> bool;
}
