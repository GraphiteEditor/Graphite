use bytemuck::{Pod, Zeroable};
use graph_craft::proto::ProtoNetwork;
use graphene_core::*;

use anyhow::Result;
use dyn_any::{StaticType, StaticTypeSized};
use futures::Future;
use glam::UVec3;
use graphene_core::raster::{Image, Pixel, SRGBA8};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::pin::Pin;
use std::sync::Arc;

type ReadBackFuture = Pin<Box<dyn Future<Output = Result<Vec<u8>>>>>;

pub enum ComputePassDimensions {
	X(u32),
	XY(u32, u32),
	XYZ(u32, u32, u32),
}

impl ComputePassDimensions {
	pub fn get(&self) -> (u32, u32, u32) {
		match self {
			ComputePassDimensions::X(x) => (*x, 1, 1),
			ComputePassDimensions::XY(x, y) => (*x, *y, 1),
			ComputePassDimensions::XYZ(x, y, z) => (*x, *y, *z),
		}
	}
}

pub trait Texture {
	fn width(&self) -> u32;
	fn height(&self) -> u32;
	fn format(&self) -> TextureBufferType;
	fn view<TextureView>(&self) -> TextureView;
}

pub trait GpuExecutor {
	type ShaderHandle;
	type BufferHandle;
	type TextureHandle;
	type TextureView;
	type CommandBuffer;

	fn load_shader(&self, shader: Shader) -> Result<Self::ShaderHandle>;
	fn create_uniform_buffer<T: ToUniformBuffer>(&self, data: T) -> Result<ShaderInput<Self>>;
	fn create_storage_buffer<T: ToStorageBuffer>(&self, data: T, options: StorageBufferOptions) -> Result<ShaderInput<Self>>;
	fn create_texture_buffer<T: ToTextureBuffer>(&self, data: T, options: TextureBufferOptions) -> Result<ShaderInput<Self>>;
	fn create_texture_view(&self, texture: ShaderInput<Self>) -> Result<ShaderInput<Self>>;
	fn create_output_buffer(&self, len: usize, ty: Type, cpu_readable: bool) -> Result<ShaderInput<Self>>;
	fn create_compute_pass(&self, layout: &PipelineLayout<Self>, read_back: Option<Arc<ShaderInput<Self>>>, instances: ComputePassDimensions) -> Result<Self::CommandBuffer>;
	fn create_render_pass(&self, texture: ShaderInput<Self>, canvas: ShaderInput<Self>) -> Result<Self::CommandBuffer>;
	fn execute_compute_pipeline(&self, encoder: Self::CommandBuffer) -> Result<()>;
	fn read_output_buffer(&self, buffer: Arc<ShaderInput<Self>>) -> ReadBackFuture;
}

pub trait SpirVCompiler {
	fn compile(&self, network: &[ProtoNetwork], io: &ShaderIO) -> Result<Shader>;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompileRequest {
	pub networks: Vec<ProtoNetwork>,
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
pub struct DummyExecutor;

impl GpuExecutor for DummyExecutor {
	type ShaderHandle = ();
	type BufferHandle = ();
	type TextureHandle = ();
	type TextureView = ();
	type CommandBuffer = ();

	fn load_shader(&self, _shader: Shader) -> Result<Self::ShaderHandle> {
		todo!()
	}

	fn create_uniform_buffer<T: ToUniformBuffer>(&self, _data: T) -> Result<ShaderInput<Self>> {
		todo!()
	}

	fn create_storage_buffer<T: ToStorageBuffer>(&self, _data: T, _options: StorageBufferOptions) -> Result<ShaderInput<Self>> {
		todo!()
	}

	fn create_texture_buffer<T: ToTextureBuffer>(&self, _data: T, _options: TextureBufferOptions) -> Result<ShaderInput<Self>> {
		todo!()
	}

	fn create_output_buffer(&self, _len: usize, _ty: Type, _cpu_readable: bool) -> Result<ShaderInput<Self>> {
		todo!()
	}

	fn create_compute_pass(&self, _layout: &PipelineLayout<Self>, _read_back: Option<Arc<ShaderInput<Self>>>, _instances: ComputePassDimensions) -> Result<Self::CommandBuffer> {
		todo!()
	}

	fn execute_compute_pipeline(&self, _encoder: Self::CommandBuffer) -> Result<()> {
		todo!()
	}

	fn create_render_pass(&self, _texture: ShaderInput<Self>, _canvas: ShaderInput<Self>) -> Result<Self::CommandBuffer> {
		todo!()
	}

	fn read_output_buffer(&self, _buffer: Arc<ShaderInput<Self>>) -> ReadBackFuture {
		todo!()
	}

	fn create_texture_view(&self, _texture: ShaderInput<Self>) -> Result<ShaderInput<Self>> {
		todo!()
	}
}

type AbstractShaderInput = ShaderInput<DummyExecutor>;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
/// All the possible inputs to a shader.
pub enum ShaderInput<E: GpuExecutor + ?Sized> {
	UniformBuffer(E::BufferHandle, Type),
	StorageBuffer(E::BufferHandle, Type),
	TextureBuffer(E::TextureHandle, Type),
	StorageTextureBuffer(E::TextureHandle, Type),
	/// A struct representing a work group memory buffer. This cannot be accessed by the CPU.
	WorkGroupMemory(usize, Type),
	Constant(GPUConstant),
	OutputBuffer(E::BufferHandle, Type),
	ReadBackBuffer(E::BufferHandle, Type),
}

pub enum BindingType<'a, E: GpuExecutor> {
	UniformBuffer(&'a E::BufferHandle),
	StorageBuffer(&'a E::BufferHandle),
	Texture(&'a E::TextureHandle),
}

/// Extract the buffer handle from a shader input.
impl<E: GpuExecutor> ShaderInput<E> {
	pub fn buffer(&self) -> Option<BindingType<E>> {
		match self {
			ShaderInput::UniformBuffer(buffer, _) => Some(BindingType::UniformBuffer(buffer)),
			ShaderInput::StorageBuffer(buffer, _) => Some(BindingType::StorageBuffer(buffer)),
			ShaderInput::WorkGroupMemory(_, _) => None,
			ShaderInput::Constant(_) => None,
			ShaderInput::TextureBuffer(tex, _) => Some(BindingType::Texture(tex)),
			ShaderInput::StorageTextureBuffer(_, _) => None,
			ShaderInput::OutputBuffer(buffer, _) => Some(BindingType::StorageBuffer(buffer)),
			ShaderInput::ReadBackBuffer(buffer, _) => Some(BindingType::StorageBuffer(buffer)),
		}
	}
	pub fn ty(&self) -> Type {
		match self {
			ShaderInput::UniformBuffer(_, ty) => ty.clone(),
			ShaderInput::StorageBuffer(_, ty) => ty.clone(),
			ShaderInput::WorkGroupMemory(_, ty) => ty.clone(),
			ShaderInput::Constant(c) => c.ty(),
			ShaderInput::TextureBuffer(_, ty) => ty.clone(),
			ShaderInput::StorageTextureBuffer(_, ty) => ty.clone(),
			ShaderInput::TextureView(_, ty) => ty.clone(),
			ShaderInput::OutputBuffer(_, ty) => ty.clone(),
			ShaderInput::ReadBackBuffer(_, ty) => ty.clone(),
		}
	}

	pub fn is_output(&self) -> bool {
		matches!(self, ShaderInput::OutputBuffer(_, _))
	}
}

pub struct Shader<'a> {
	pub source: Cow<'a, [u32]>,
	pub name: &'a str,
	pub io: ShaderIO,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShaderIO {
	pub inputs: Vec<AbstractShaderInput>,
	pub output: AbstractShaderInput,
}

pub struct StorageBufferOptions {
	pub cpu_writable: bool,
	pub gpu_writable: bool,
	pub cpu_readable: bool,
	pub storage: bool,
}

pub enum TextureBufferOptions {
	Storage,
	Texture,
	Surface,
}

pub trait ToUniformBuffer: StaticType {
	fn to_bytes(&self) -> Cow<[u8]>;
}

impl<T: StaticType + Pod + Zeroable> ToUniformBuffer for T {
	fn to_bytes(&self) -> Cow<[u8]> {
		Cow::Owned(bytemuck::bytes_of(self).into())
	}
}

pub trait ToStorageBuffer: StaticType {
	fn to_bytes(&self) -> Cow<[u8]>;
	fn ty(&self) -> Type;
}

impl<T: Pod + Zeroable + StaticTypeSized> ToStorageBuffer for Vec<T> {
	fn to_bytes(&self) -> Cow<[u8]> {
		Cow::Borrowed(bytemuck::cast_slice(self.as_slice()))
	}
	fn ty(&self) -> Type {
		concrete!(T)
	}
}

pub trait TextureFormat {
	fn format() -> TextureBufferType;
}

impl TextureFormat for Color {
	fn format() -> TextureBufferType {
		TextureBufferType::Rgba32Float
	}
}
impl TextureFormat for SRGBA8 {
	fn format() -> TextureBufferType {
		TextureBufferType::Rgba8Srgb
	}
}

pub enum TextureBufferType {
	Rgba32Float,
	Rgba8Srgb,
}

pub trait ToTextureBuffer: StaticType {
	fn to_bytes(&self) -> Cow<[u8]>;
	fn ty() -> Type;
	fn format() -> TextureBufferType;
	fn size(&self) -> (u32, u32);
}

impl<T: Pod + Zeroable + StaticTypeSized + Pixel + TextureFormat> ToTextureBuffer for Image<T>
where
	T::Static: Pixel,
{
	fn to_bytes(&self) -> Cow<[u8]> {
		Cow::Borrowed(bytemuck::cast_slice(self.data.as_slice()))
	}
	fn ty() -> Type {
		concrete!(T)
	}
	fn format() -> TextureBufferType {
		T::format()
	}
	fn size(&self) -> (u32, u32) {
		(self.width, self.height)
	}
}

/// Collection of all arguments that are passed to the shader.
pub struct Bindgroup<E: GpuExecutor + ?Sized> {
	pub buffers: Vec<Arc<ShaderInput<E>>>,
}

/// A struct representing a compute pipeline.
pub struct PipelineLayout<E: GpuExecutor + ?Sized> {
	pub shader: E::ShaderHandle,
	pub entry_point: String,
	pub bind_group: Bindgroup<E>,
	pub output_buffer: Arc<ShaderInput<E>>,
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
fn uniform_node<T: ToUniformBuffer, E: GpuExecutor>(data: T, executor: &'input E) -> ShaderInput<E> {
	executor.create_uniform_buffer(data).unwrap()
}

pub struct StorageNode<Executor> {
	executor: Executor,
}

#[node_macro::node_fn(StorageNode)]
fn storage_node<T: ToStorageBuffer, E: GpuExecutor>(data: T, executor: &'input E) -> ShaderInput<E> {
	executor
		.create_storage_buffer(
			data,
			StorageBufferOptions {
				cpu_writable: false,
				gpu_writable: true,
				cpu_readable: false,
				storage: true,
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
fn create_output_buffer_node<E: GpuExecutor>(size: usize, executor: &'input E, ty: Type) -> ShaderInput<E> {
	executor.create_output_buffer(size, ty, true).unwrap()
}

pub struct CreateComputePassNode<Executor, Output, Instances> {
	executor: Executor,
	output: Output,
	instances: Instances,
}

#[node_macro::node_fn(CreateComputePassNode)]
fn create_compute_pass_node<'any_input, E: 'any_input + GpuExecutor>(layout: PipelineLayout<E>, executor: &'any_input E, output: ShaderInput<E>, instances: ComputePassDimensions) -> E::CommandBuffer {
	executor.create_compute_pass(&layout, Some(output.into()), instances).unwrap()
}

pub struct CreatePipelineLayoutNode<_E, EntryPoint, Bindgroup, OutputBuffer> {
	entry_point: EntryPoint,
	bind_group: Bindgroup,
	output_buffer: OutputBuffer,
	_e: std::marker::PhantomData<_E>,
}

#[node_macro::node_fn(CreatePipelineLayoutNode<_E>)]
fn create_pipeline_layout_node<_E: GpuExecutor>(shader: _E::ShaderHandle, entry_point: String, bind_group: Bindgroup<_E>, output_buffer: Arc<ShaderInput<_E>>) -> PipelineLayout<_E> {
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

pub struct ReadOutputBufferNode<Executor> {
	executor: Executor,
}
#[node_macro::node_fn(ReadOutputBufferNode)]
async fn read_output_buffer_node<E: GpuExecutor>(buffer: Arc<ShaderInput<E>>, executor: &'input E) -> Vec<u8> {
	executor.read_output_buffer(buffer).await.unwrap()
}
