#![cfg(feature = "wgpu")]

use std::sync::Arc;
use wgpu::{Device, Instance, Queue};

#[derive(Debug, Clone)]
pub struct WgpuContext {
	pub device: Arc<Device>,
	pub queue: Arc<Queue>,
	pub instance: Arc<Instance>,
	pub adapter: Arc<wgpu::Adapter>,
}
