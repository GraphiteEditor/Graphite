use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Buffer(Arc<BufferInner>);

#[derive(Debug)]
struct BufferInner(wgpu::Buffer);

impl Drop for BufferInner {
	fn drop(&mut self) {
		self.0.destroy();
	}
}

impl Deref for Buffer {
	type Target = wgpu::Buffer;

	fn deref(&self) -> &Self::Target {
		&self.0.0
	}
}

impl AsRef<wgpu::Buffer> for Buffer {
	fn as_ref(&self) -> &wgpu::Buffer {
		&self.0.0
	}
}

impl From<wgpu::Buffer> for Buffer {
	fn from(buffer: wgpu::Buffer) -> Self {
		Self(Arc::new(BufferInner(buffer)))
	}
}
