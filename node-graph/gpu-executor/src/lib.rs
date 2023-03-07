use anyhow::Result;
use futures::Future;
use graphene_core::*;
use std::borrow::Cow;
use std::pin::Pin;

pub trait GpuExecutor {
	type ShaderHandle;
	type BufferHandle;
	type CommandBuffer;

	fn load_shader(&mut self, shader: Shader) -> Result<Self::ShaderHandle>;
	fn create_uniform_buffer<T: ToUniformBuffer>(&mut self, data: T) -> Result<Self::BufferHandle>;
	fn create_storage_buffer<T: ToStorageBuffer>(&mut self, data: T, options: StorageBufferOptions) -> Result<Self::BufferHandle>;
	fn create_output_buffer(&mut self, size: u64) -> Result<Self::BufferHandle>;
	fn create_compute_pass(&mut self, layout: &PipelineLayout<Self::ShaderHandle, Self::BufferHandle>, output: Self::BufferHandle) -> Result<Self::CommandBuffer>;
	fn execute_compute_pipeline(&mut self, encoder: Self::CommandBuffer) -> Result<()>;
	fn read_output_buffer(&mut self, buffer: Self::BufferHandle) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>>>>;
}

pub struct Shader<'a> {
	pub source: Cow<'a, [u32]>,
	pub name: &'a str,
}

pub struct StorageBufferOptions {
	pub cpu_writable: bool,
	pub gpu_writable: bool,
	pub cpu_readable: bool,
}

pub trait ToUniformBuffer {
	type UniformBufferHandle;
	fn to_bytes(&self) -> Cow<[u8]>;
}

pub trait ToStorageBuffer {
	type StorageBufferHandle;
	fn to_bytes(&self) -> Cow<[u8]>;
}

pub struct PipelineLayout<ShaderHandle, BufferHandle> {
	pub shader: ShaderHandle,
	pub entry_point: String,
	pub uniform_buffers: Vec<BufferHandle>,
	pub storage_buffers: Vec<BufferHandle>,
	pub instances: u32,
	pub output_buffer: BufferHandle,
}

// TODO: add shader input nodes
// * ShaderInputNode

/// Extracts arguments from the function arguments and wraps them in a node
pub struct ShaderInputNode<T> {
	data: T,
}

impl<'i, T: 'i> Node<'i, ()> for ShaderInputNode<T> {
	type Output = &'i T;
	fn eval<'s: 'i>(&'s self, _: ()) -> Self::Output {
		&self.data
	}
}

impl<T> ShaderInputNode<T> {
	pub fn new(data: T) -> Self {
		Self { data }
	}
}
