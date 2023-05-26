use graph_craft::proto::ProtoNetwork;
use graphene_core::*;

use anyhow::Result;
use dyn_any::StaticType;
use futures::Future;
use glam::UVec3;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::pin::Pin;

type ReadBackFuture = Pin<Box<dyn Future<Output = Result<Vec<u8>>>>>;

pub trait GpuExecutor {
	type ShaderHandle;
	type BufferHandle;
	type CommandBuffer;

	fn load_shader(&self, shader: Shader) -> Result<Self::ShaderHandle>;
	fn create_uniform_buffer<T: ToUniformBuffer>(&self, data: T) -> Result<ShaderInput<Self::BufferHandle>>;
	fn create_storage_buffer<T: ToStorageBuffer>(&self, data: T, options: StorageBufferOptions) -> Result<ShaderInput<Self::BufferHandle>>;
	fn create_output_buffer(&self, len: usize, ty: Type, cpu_readable: bool) -> Result<ShaderInput<Self::BufferHandle>>;
	fn create_compute_pass(&self, layout: &PipelineLayout<Self>, read_back: Option<ShaderInput<Self::BufferHandle>>, instances: u32) -> Result<Self::CommandBuffer>;
	fn execute_compute_pipeline(&self, encoder: Self::CommandBuffer) -> Result<()>;
	fn read_output_buffer(&self, buffer: ShaderInput<Self::BufferHandle>) -> Result<ReadBackFuture>;
}

pub trait SpirVCompiler {
	fn compile(&self, network: ProtoNetwork, io: &ShaderIO) -> Result<Shader>;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompileRequest {
	pub network: ProtoNetwork,
	pub io: ShaderIO,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
/// GPU constants that can be used as inputs to a shader.
pub enum GPUConstant {
	SubGroupId,
	SubGroupInvocationId,
	SubGroupSize,
	NumSubGroups,
	WorkGroupId,
	WorkGroupInvocationId,
	WorkGroupSize,
	NumWorkGroups,
	GlobalInvocationId,
	GlobalSize,
}

impl GPUConstant {
	pub fn ty(&self) -> Type {
		match self {
			GPUConstant::SubGroupId => concrete!(u32),
			GPUConstant::SubGroupInvocationId => concrete!(u32),
			GPUConstant::SubGroupSize => concrete!(u32),
			GPUConstant::NumSubGroups => concrete!(u32),
			GPUConstant::WorkGroupId => concrete!(UVec3),
			GPUConstant::WorkGroupInvocationId => concrete!(UVec3),
			GPUConstant::WorkGroupSize => concrete!(u32),
			GPUConstant::NumWorkGroups => concrete!(u32),
			GPUConstant::GlobalInvocationId => concrete!(UVec3),
			GPUConstant::GlobalSize => concrete!(UVec3),
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
/// All the possible inputs to a shader.
pub enum ShaderInput<BufferHandle> {
	UniformBuffer(BufferHandle, Type),
	StorageBuffer(BufferHandle, Type),
	/// A struct representing a work group memory buffer. This cannot be accessed by the CPU.
	WorkGroupMemory(usize, Type),
	Constant(GPUConstant),
	OutputBuffer(BufferHandle, Type),
	ReadBackBuffer(BufferHandle, Type),
}

/// Extract the buffer handle from a shader input.
impl<BufferHandle> ShaderInput<BufferHandle> {
	pub fn buffer(&self) -> Option<&BufferHandle> {
		match self {
			ShaderInput::UniformBuffer(buffer, _) => Some(buffer),
			ShaderInput::StorageBuffer(buffer, _) => Some(buffer),
			ShaderInput::WorkGroupMemory(_, _) => None,
			ShaderInput::Constant(_) => None,
			ShaderInput::OutputBuffer(buffer, _) => Some(buffer),
			ShaderInput::ReadBackBuffer(buffer, _) => Some(buffer),
		}
	}
	pub fn ty(&self) -> Type {
		match self {
			ShaderInput::UniformBuffer(_, ty) => ty.clone(),
			ShaderInput::StorageBuffer(_, ty) => ty.clone(),
			ShaderInput::WorkGroupMemory(_, ty) => ty.clone(),
			ShaderInput::Constant(c) => c.ty(),
			ShaderInput::OutputBuffer(_, ty) => ty.clone(),
			ShaderInput::ReadBackBuffer(_, ty) => ty.clone(),
		}
	}
}

pub struct Shader<'a> {
	pub source: Cow<'a, [u32]>,
	pub name: &'a str,
	pub io: ShaderIO,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShaderIO {
	pub inputs: Vec<ShaderInput<()>>,
	pub output: ShaderInput<()>,
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

/// Collection of all arguments that are passed to the shader.
pub struct Bindgroup<E: GpuExecutor + ?Sized> {
	pub buffers: Vec<ShaderInput<E::BufferHandle>>,
}

/// A struct representing a compute pipeline.
pub struct PipelineLayout<E: GpuExecutor + ?Sized> {
	pub shader: E::ShaderHandle,
	pub entry_point: String,
	pub bind_group: Bindgroup<E>,
	pub output_buffer: ShaderInput<E::BufferHandle>,
}

/// Extracts arguments from the function arguments and wraps them in a node.
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
fn uniform_node<T: ToUniformBuffer, E: GpuExecutor>(data: T, executor: &'input E) -> ShaderInput<E::BufferHandle> {
	executor.create_uniform_buffer(data).unwrap()
}

pub struct StorageNode<Executor> {
	executor: Executor,
}

#[node_macro::node_fn(StorageNode)]
fn storage_node<T: ToStorageBuffer, E: GpuExecutor>(data: T, executor: &'input E) -> ShaderInput<E::BufferHandle> {
	executor
		.create_storage_buffer(
			data,
			StorageBufferOptions {
				cpu_writable: false,
				gpu_writable: true,
				cpu_readable: false,
			},
		)
		.unwrap()
}

pub struct PushNode<Value> {
	value: Value,
}

#[node_macro::node_fn(PushNode)]
fn push_node<T>(mut vec: Vec<T>, value: T) {
	vec.push(value);
}

pub struct CreateOutputBufferNode<Executor, Ty> {
	executor: Executor,
	ty: Ty,
}

#[node_macro::node_fn(CreateOutputBufferNode)]
fn create_output_buffer_node<E: GpuExecutor>(size: usize, executor: &'input E, ty: Type) -> ShaderInput<E::BufferHandle> {
	executor.create_output_buffer(size, ty, true).unwrap()
}

pub struct CreateComputePassNode<Executor, Output, Instances> {
	executor: Executor,
	output: Output,
	instances: Instances,
}

#[node_macro::node_fn(CreateComputePassNode)]
fn create_compute_pass_node<E: GpuExecutor>(layout: PipelineLayout<E>, executor: &'input E, output: ShaderInput<E::BufferHandle>, instances: u32) -> E::CommandBuffer {
	executor.create_compute_pass(&layout, Some(output), instances).unwrap()
}

pub struct CreatePipelineLayoutNode<_E, EntryPoint, Bindgroup, OutputBuffer> {
	entry_point: EntryPoint,
	bind_group: Bindgroup,
	output_buffer: OutputBuffer,
	_e: std::marker::PhantomData<_E>,
}

#[node_macro::node_fn(CreatePipelineLayoutNode<_E>)]
fn create_pipeline_layout_node<_E: GpuExecutor>(shader: _E::ShaderHandle, entry_point: String, bind_group: Bindgroup<_E>, output_buffer: ShaderInput<_E::BufferHandle>) -> PipelineLayout<_E> {
	PipelineLayout {
		shader,
		entry_point,
		bind_group,
		output_buffer,
	}
}

pub struct ExecuteComputePipelineNode<Executor> {
	executor: Executor,
}

#[node_macro::node_fn(ExecuteComputePipelineNode)]
fn execute_compute_pipeline_node<E: GpuExecutor>(encoder: E::CommandBuffer, executor: &'input mut E) {
	executor.execute_compute_pipeline(encoder).unwrap();
}

// TODO
// pub struct ReadOutputBufferNode<Executor> {
// 	executor: Executor,
// }
// #[node_macro::node_fn(ReadOutputBufferNode)]
// fn read_output_buffer_node<E: GpuExecutor>(buffer: E::BufferHandle, executor: &'input mut E) -> Vec<u8> {
// 	executor.read_output_buffer(buffer).await.unwrap()
// }
