//! Wraps wgpu types to provide synchronization against surface configuration.
//! Everything sharing a [`Instance`] is synchronized against that instance's [`Surface`]s.
//! [`Surface::configure`] takes a write lock, and all other operations take a read lock.
//!
//! [`wgpu::Surface::configure`] recreates the swapchain and waits for the GPU to idle.
//! A concurrent `submit`, `get_current_texture`, or `present` makes that
//! wait fail (validation error, panic, or driver crash on the unsafe hal usage).
//!
//! [`Instance`] and [`Adapter`] wrapper types can be dereferenced to the underlying wgpu type.
//! Their `create_surface`/`request_adapter`/`request_device` methods shadow the wgpu ones.
//! These methods return wrapper types that synchronize against the parent [`Instance`].
//! Be aware that using the underlying wgpu versions directly (through deref) results in unsynchronized objects.
//!
//! Guards hold their read lock for their whole lifetime.
//! While holding a [`SurfaceTextureGuard`] reuse its [`QueueGuard`] via [`SurfaceTextureGuard::queue`] to avoid deadlock.

use std::ops::Deref;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Clone, Debug)]
struct Lock(Arc<RwLock<()>>);

impl Lock {
	fn new() -> Self {
		Self(Arc::new(RwLock::new(())))
	}

	fn read(&self) -> RwLockReadGuard<'_, ()> {
		self.0.read().unwrap_or_else(PoisonError::into_inner)
	}

	fn write(&self) -> RwLockWriteGuard<'_, ()> {
		self.0.write().unwrap_or_else(PoisonError::into_inner)
	}
}

#[derive(Clone, Debug)]
pub struct Instance {
	raw: wgpu::Instance,
	lock: Lock,
}

impl Instance {
	pub fn new(raw: wgpu::Instance) -> Self {
		Self { raw, lock: Lock::new() }
	}

	pub fn create_surface(&self, target: impl Into<wgpu::SurfaceTarget<'static>>) -> Result<Surface, wgpu::CreateSurfaceError> {
		Ok(Surface {
			raw: self.raw.create_surface(target)?,
			lock: self.lock.clone(),
		})
	}

	pub async fn request_adapter(&self, options: &wgpu::RequestAdapterOptions<'_, '_>) -> Result<Adapter, wgpu::RequestAdapterError> {
		Ok(Adapter {
			raw: self.raw.request_adapter(options).await?,
			lock: self.lock.clone(),
		})
	}

	pub async fn enumerate_adapters(&self, backends: wgpu::Backends) -> Vec<Adapter> {
		self.raw
			.enumerate_adapters(backends)
			.await
			.into_iter()
			.map(|adapter| Adapter {
				raw: adapter,
				lock: self.lock.clone(),
			})
			.collect()
	}
}

impl Deref for Instance {
	type Target = wgpu::Instance;
	fn deref(&self) -> &wgpu::Instance {
		&self.raw
	}
}

#[derive(Clone, Debug)]
pub struct Adapter {
	raw: wgpu::Adapter,
	lock: Lock,
}

impl Adapter {
	pub async fn request_device(&self, desc: &wgpu::DeviceDescriptor<'_>) -> Result<(wgpu::Device, Queue), wgpu::RequestDeviceError> {
		let (device, queue) = self.raw.request_device(desc).await?;
		Ok((device, Queue { raw: queue, lock: self.lock.clone() }))
	}
}

impl Deref for Adapter {
	type Target = wgpu::Adapter;
	fn deref(&self) -> &wgpu::Adapter {
		&self.raw
	}
}

#[derive(Clone, Debug)]
pub struct Queue {
	raw: wgpu::Queue,
	lock: Lock,
}

impl Queue {
	pub fn submit<I: IntoIterator<Item = wgpu::CommandBuffer>>(&self, command_buffers: I) -> wgpu::SubmissionIndex {
		self.lock().submit(command_buffers)
	}

	pub fn lock(&self) -> QueueGuard<'_> {
		QueueGuard {
			raw: &self.raw,
			_guard: self.lock.read(),
		}
	}

	pub fn write_buffer(&self, buffer: &wgpu::Buffer, offset: wgpu::BufferAddress, data: &[u8]) {
		self.lock().write_buffer(buffer, offset, data);
	}

	pub fn write_texture(&self, texture: wgpu::TexelCopyTextureInfo<'_>, data: &[u8], data_layout: wgpu::TexelCopyBufferLayout, size: wgpu::Extent3d) {
		self.lock().write_texture(texture, data, data_layout, size);
	}
}

pub struct QueueGuard<'a> {
	raw: &'a wgpu::Queue,
	_guard: RwLockReadGuard<'a, ()>,
}

impl Deref for QueueGuard<'_> {
	type Target = wgpu::Queue;
	fn deref(&self) -> &wgpu::Queue {
		self.raw
	}
}

#[derive(Debug)]
pub struct Surface {
	raw: wgpu::Surface<'static>,
	lock: Lock,
}

impl Surface {
	pub fn configure(&self, device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) {
		let _guard = self.lock.write();
		self.raw.configure(device, config);
	}

	pub fn get_current_texture<'a>(&self, queue: &'a Queue) -> CurrentSurfaceTexture<'a> {
		debug_assert!(Arc::ptr_eq(&self.lock.0, &queue.lock.0), "queue must come from the same `Instance` as this surface");
		let guard = queue.lock();
		let raw = self.raw.get_current_texture();
		match raw {
			wgpu::CurrentSurfaceTexture::Success(raw) => CurrentSurfaceTexture::Success(SurfaceTextureGuard { raw, queue: guard }),
			wgpu::CurrentSurfaceTexture::Suboptimal(raw) => CurrentSurfaceTexture::Suboptimal(SurfaceTextureGuard { raw, queue: guard }),
			wgpu::CurrentSurfaceTexture::Occluded => CurrentSurfaceTexture::Occluded,
			wgpu::CurrentSurfaceTexture::Lost => CurrentSurfaceTexture::Lost,
			wgpu::CurrentSurfaceTexture::Outdated => CurrentSurfaceTexture::Outdated,
			wgpu::CurrentSurfaceTexture::Timeout => CurrentSurfaceTexture::Timeout,
			wgpu::CurrentSurfaceTexture::Validation => CurrentSurfaceTexture::Validation,
		}
	}

	pub fn get_capabilities(&self, adapter: &wgpu::Adapter) -> wgpu::SurfaceCapabilities {
		self.raw.get_capabilities(adapter)
	}
}

#[derive(Debug)]
pub enum CurrentSurfaceTexture<'a> {
	Success(SurfaceTextureGuard<'a>),
	Suboptimal(SurfaceTextureGuard<'a>),
	Occluded,
	Lost,
	Outdated,
	Timeout,
	Validation,
}

pub struct SurfaceTextureGuard<'a> {
	raw: wgpu::SurfaceTexture,
	pub queue: QueueGuard<'a>,
}

impl SurfaceTextureGuard<'_> {
	pub fn present(self) {
		self.raw.present();
	}
}

impl Deref for SurfaceTextureGuard<'_> {
	type Target = wgpu::SurfaceTexture;
	fn deref(&self) -> &Self::Target {
		&self.raw
	}
}

impl std::fmt::Debug for SurfaceTextureGuard<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SurfaceTexture").field("raw", &self.raw).finish()
	}
}
