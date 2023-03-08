use anyhow::Result;
use dyn_any::StaticType;
use futures::Future;
use graphene_core::*;
use std::borrow::Cow;
use std::pin::Pin;

pub trait GpuExecutor {
	type ShaderHandle;
	type BufferHandle;
	type CommandBuffer;

	fn load_shader(&self, shader: Shader) -> Result<Self::ShaderHandle>;
	fn create_uniform_buffer<T: ToUniformBuffer>(&self, data: T) -> Result<Self::BufferHandle>;
	fn create_storage_buffer<T: ToStorageBuffer>(&self, data: T, options: StorageBufferOptions) -> Result<Self::BufferHandle>;
	fn create_output_buffer(&self, size: u64) -> Result<Self::BufferHandle>;
	fn create_compute_pass(&self, layout: &PipelineLayout<Self::ShaderHandle, Self::BufferHandle>, output: Self::BufferHandle) -> Result<Self::CommandBuffer>;
	fn execute_compute_pipeline(&self, encoder: Self::CommandBuffer) -> Result<()>;
	fn read_output_buffer(&self, buffer: Self::BufferHandle) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>>>>;
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

pub trait ToUniformBuffer: StaticType {
	type UniformBufferHandle;
	fn to_bytes(&self) -> Cow<[u8]>;
}

pub trait ToStorageBuffer: StaticType {
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
	fn eval(&'i self, _: ()) -> Self::Output {
		&self.data
	}
}

impl<T> ShaderInputNode<T> {
	pub fn new(data: T) -> Self {
		Self { data }
	}
}

pub struct UniformNode<Executor> {
	executor: Executor,
}

#[node_macro::node_fn(UniformNode)]
fn uniform_node<T: ToUniformBuffer, E: GpuExecutor>(data: T, executor: &'any_input mut E) -> (Type, E::BufferHandle) {
	let handle = executor.create_uniform_buffer(data).unwrap();
	(Type::new::<T>(), handle)
}

pub struct StorageNode<Executor> {
	executor: Executor,
}

#[node_macro::node_fn(StorageNode)]
fn storage_node<T: ToStorageBuffer, E: GpuExecutor>(data: T, executor: &'any_input mut E) -> (Type, E::BufferHandle) {
	let handle = executor
		.create_storage_buffer(
			data,
			StorageBufferOptions {
				cpu_writable: false,
				gpu_writable: true,
				cpu_readable: false,
			},
		)
		.unwrap();
	(Type::new::<T>(), handle)
}

pub struct PushNode<Value> {
	value: Value,
}

#[node_macro::node_fn(PushNode)]
fn push_node<T>(mut vec: Vec<T>, value: T) {
	vec.push(value);
}

pub struct CreateOutputBufferNode<Executor> {
	executor: Executor,
}

#[node_macro::node_fn(CreateOutputBufferNode)]
fn create_output_buffer_node<E: GpuExecutor>(size: u64, executor: &'any_input mut E) -> E::BufferHandle {
	executor.create_output_buffer(size).unwrap()
}

//TODO: add mul
// * MulNode
// * TypeSizeNode
