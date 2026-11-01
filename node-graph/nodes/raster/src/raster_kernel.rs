use core_types::ATTR_TRANSFORM;
use core_types::color::Color;
use core_types::list::Item;
use core_types::ops::ConvertAsync;
use core_types::transform::Footprint;
use core_types::{Ctx, NodeError};
use glam::{DAffine2, DMat2, DVec2, UVec2};
use raster_types::{CPU, GPU, ImageError, Raster, Storage};
use std::any::{TypeId, type_name};
use vector_types::Gradient;
pub use wgpu_executor::shader_runtime::raster_kernel_runtime::RasterKernelLookup;
use wgpu_executor::shader_runtime::raster_kernel_runtime::{RasterKernelArgs, RasterKernelInput, RasterKernelRuntime};
use wgpu_executor::{RASTER_FORMAT, WgpuExecutor, WgpuPipelineCache, WgpuTextureFormat};

/// The WGSL of a [`MapColor`] or [`RasterKernel`], which sees the shared prelude and the `Params` struct with its `params()` accessor.
pub enum Wgsl {
	/// The body of the kernel's function, with every parameter bound as a local.
	Body(String),
	/// A source defining the kernel's function and any helpers it calls.
	Function(String),
}

impl Wgsl {
	pub fn body(body: impl Into<String>) -> Self {
		Self::Body(body.into())
	}

	pub fn function(source: impl Into<String>) -> Self {
		Self::Function(source.into())
	}
}

/// A parameter struct passed to a [`MapColor`] or [`RasterKernel`] shader, declared with [`gpu_params!`].
pub trait GpuParams {
	fn write(&self, slots: &mut Vec<u32>);
	/// The WGSL declarations of the struct and its nested structs, aliased as `Params`, and the `params()` accessor, or nothing for a struct without fields.
	fn wgsl() -> String;
	/// The WGSL statements binding every field of `params()` as a local.
	fn wgsl_locals() -> String;
	/// The WGSL declaring the texture and accessors of each lookup field, bound in order from `binding`.
	fn wgsl_lookups(_binding: usize) -> String {
		String::new()
	}
	/// The tables of the lookup fields, in order.
	fn lookups(&self) -> Vec<RasterKernelLookup<'_>> {
		Vec::new()
	}
}

/// A field type of a [`gpu_params!`] struct, occupying `SLOTS` 32-bit uniform slots.
pub trait GpuParamField {
	const SLOTS: usize;
	/// Whether the type is a lookup table, bound as its own texture rather than packed into slots, which only a top-level field may be.
	const LOOKUP: bool = false;
	fn wgsl_type() -> String;
	fn write(&self, slots: &mut Vec<u32>);
	fn wgsl_read(slot: usize) -> String;
	/// Appends the WGSL struct declarations this type needs, each only once.
	fn wgsl_declarations(_declarations: &mut Vec<String>) {}
	/// The WGSL declaring a lookup table's texture at `binding` and its accessors named after `field`.
	fn wgsl_lookup(_field: &str, _binding: usize) -> String {
		String::new()
	}
	fn lookup<'a>(&'a self, _lookups: &mut Vec<RasterKernelLookup<'a>>) {}
}

fn wgsl_slot(slot: usize) -> String {
	format!("uniform_params[{}][{}]", slot / 4, slot % 4)
}

impl GpuParamField for f32 {
	const SLOTS: usize = 1;
	fn wgsl_type() -> String {
		"f32".into()
	}
	fn write(&self, slots: &mut Vec<u32>) {
		slots.push(self.to_bits());
	}
	fn wgsl_read(slot: usize) -> String {
		format!("bitcast<f32>({})", wgsl_slot(slot))
	}
}

impl GpuParamField for u32 {
	const SLOTS: usize = 1;
	fn wgsl_type() -> String {
		"u32".into()
	}
	fn write(&self, slots: &mut Vec<u32>) {
		slots.push(*self);
	}
	fn wgsl_read(slot: usize) -> String {
		wgsl_slot(slot)
	}
}

impl GpuParamField for i32 {
	const SLOTS: usize = 1;
	fn wgsl_type() -> String {
		"i32".into()
	}
	fn write(&self, slots: &mut Vec<u32>) {
		slots.push(*self as u32);
	}
	fn wgsl_read(slot: usize) -> String {
		format!("bitcast<i32>({})", wgsl_slot(slot))
	}
}

impl GpuParamField for bool {
	const SLOTS: usize = 1;
	fn wgsl_type() -> String {
		"bool".into()
	}
	fn write(&self, slots: &mut Vec<u32>) {
		slots.push(*self as u32);
	}
	fn wgsl_read(slot: usize) -> String {
		format!("{} != 0u", wgsl_slot(slot))
	}
}

impl GpuParamField for Color {
	const SLOTS: usize = 4;
	fn wgsl_type() -> String {
		"vec4<f32>".into()
	}
	fn write(&self, slots: &mut Vec<u32>) {
		slots.extend(self.to_vec4().to_array().map(f32::to_bits));
	}
	fn wgsl_read(slot: usize) -> String {
		let channels = (slot..slot + 4).map(<f32 as GpuParamField>::wgsl_read).collect::<Vec<_>>();
		format!("vec4<f32>({})", channels.join(", "))
	}
}

impl<T: GpuParamField, const N: usize> GpuParamField for [T; N] {
	const SLOTS: usize = N * T::SLOTS;
	fn wgsl_type() -> String {
		format!("array<{}, {N}>", T::wgsl_type())
	}
	fn write(&self, slots: &mut Vec<u32>) {
		self.iter().for_each(|element| element.write(slots));
	}
	fn wgsl_read(slot: usize) -> String {
		let elements = (0..N).map(|index| T::wgsl_read(slot + index * T::SLOTS)).collect::<Vec<_>>();
		format!("{}({})", Self::wgsl_type(), elements.join(", "))
	}
	fn wgsl_declarations(declarations: &mut Vec<String>) {
		T::wgsl_declarations(declarations);
	}
}

/// A lookup field of entries spread evenly across 0 to 1, read in WGSL by `fn <field>(t: f32) -> vec4<f32>`, which clamps `t` and interpolates linearly between the nearest two entries.
///
/// Holds 1 to 4096 entries, the texture width every device supports.
#[derive(Debug, Clone, PartialEq)]
pub struct Lut1d(pub Vec<Color>);

impl GpuParamField for Lut1d {
	const SLOTS: usize = 0;
	const LOOKUP: bool = true;
	fn wgsl_type() -> String {
		String::new()
	}
	fn write(&self, _: &mut Vec<u32>) {}
	fn wgsl_read(_: usize) -> String {
		String::new()
	}
	fn wgsl_lookup(field: &str, binding: usize) -> String {
		include_str!("wgsl/lut_1d.wgsl").replace("LUT", field).replace("BINDING", &binding.to_string())
	}
	fn lookup<'a>(&'a self, lookups: &mut Vec<RasterKernelLookup<'a>>) {
		assert!((1..=4096).contains(&self.0.len()), "A 1D lookup table holds 1 to 4096 entries");
		lookups.push(RasterKernelLookup::Lut1d(&self.0));
	}
}

/// A lookup field of a `size` cubed grid of entries, red fastest, then green, then blue, read in WGSL by `fn <field>_load(index: vec3<i32>) -> vec4<f32>`,
/// which clamps the index to the grid, and `fn <field>_size() -> i32`.
#[derive(Debug, Clone, PartialEq)]
pub struct Lut3d {
	pub size: u32,
	pub data: Vec<Color>,
}

impl GpuParamField for Lut3d {
	const SLOTS: usize = 0;
	const LOOKUP: bool = true;
	fn wgsl_type() -> String {
		String::new()
	}
	fn write(&self, _: &mut Vec<u32>) {}
	fn wgsl_read(_: usize) -> String {
		String::new()
	}
	fn wgsl_lookup(field: &str, binding: usize) -> String {
		include_str!("wgsl/lut_3d.wgsl").replace("LUT", field).replace("BINDING", &binding.to_string())
	}
	fn lookup<'a>(&'a self, lookups: &mut Vec<RasterKernelLookup<'a>>) {
		assert_eq!(self.data.len(), self.size.pow(3) as usize, "A 3D lookup table holds `size` cubed entries");
		lookups.push(RasterKernelLookup::Lut3d { size: self.size, data: &self.data });
	}
}

/// Declares a struct usable as [`GpuParams`] and as a nested [`GpuParamField`], generating its packing and WGSL unpacking.
///
/// The WGSL struct takes the Rust struct's name, so it must not collide with other WGSL names in the kernel.
/// Lookup fields such as [`Lut1d`] and [`Lut3d`] stay out of the WGSL struct, read instead through accessors named after the field.
#[macro_export]
macro_rules! gpu_params {
	($(#[$attribute:meta])* $vis:vis struct $name:ident { $($(#[$field_attribute:meta])* $field_vis:vis $field:ident: $ty:ty),* $(,)? }) => {
		$(#[$attribute])*
		$vis struct $name {
			$($(#[$field_attribute])* $field_vis $field: $ty),*
		}

		impl $crate::raster_kernel::GpuParamField for $name {
			const SLOTS: usize = 0 $(+ <$ty as $crate::raster_kernel::GpuParamField>::SLOTS)*;
			fn wgsl_type() -> String {
				stringify!($name).into()
			}

			#[allow(unused_variables)]
			fn write(&self, slots: &mut Vec<u32>) {
				$(<$ty as $crate::raster_kernel::GpuParamField>::write(&self.$field, slots);)*
			}

			#[allow(unused_mut, unused_variables, unused_assignments)]
			fn wgsl_read(slot: usize) -> String {
				let mut slot = slot;
				let reads: Vec<(bool, String)> = vec![$({
					let read = <$ty as $crate::raster_kernel::GpuParamField>::wgsl_read(slot);
					slot += <$ty as $crate::raster_kernel::GpuParamField>::SLOTS;
					(<$ty as $crate::raster_kernel::GpuParamField>::LOOKUP, read)
				}),*];
				let reads: Vec<String> = reads.into_iter().filter(|(lookup, _)| !lookup).map(|(_, read)| read).collect();
				format!("{}({})", stringify!($name), reads.join(", "))
			}

			#[allow(unused_variables)]
			fn wgsl_declarations(declarations: &mut Vec<String>) {
				$(<$ty as $crate::raster_kernel::GpuParamField>::wgsl_declarations(declarations);)*
				let fields: Vec<(bool, String)> = vec![$((
					<$ty as $crate::raster_kernel::GpuParamField>::LOOKUP,
					format!("{}: {}", stringify!($field), <$ty as $crate::raster_kernel::GpuParamField>::wgsl_type())
				)),*];
				let fields: Vec<String> = fields.into_iter().filter(|(lookup, _)| !lookup).map(|(_, field)| field).collect();
				if fields.is_empty() {
					return;
				}
				let declaration = format!("struct {} {{\n\t{},\n}}\n", stringify!($name), fields.join(",\n\t"));
				if !declarations.contains(&declaration) {
					declarations.push(declaration);
				}
			}
		}

		impl $crate::raster_kernel::GpuParams for $name {
			fn write(&self, slots: &mut Vec<u32>) {
				<Self as $crate::raster_kernel::GpuParamField>::write(self, slots);
			}

			fn wgsl() -> String {
				let mut declarations = Vec::new();
				<Self as $crate::raster_kernel::GpuParamField>::wgsl_declarations(&mut declarations);
				if declarations.is_empty() {
					return String::new();
				}
				format!(
					"{}\nalias Params = {};\n\nfn params() -> Params {{\n\treturn {};\n}}\n",
					declarations.join("\n"),
					stringify!($name),
					<Self as $crate::raster_kernel::GpuParamField>::wgsl_read(0)
				)
			}

			fn wgsl_locals() -> String {
				let fields: &[(bool, &str)] = &[$((<$ty as $crate::raster_kernel::GpuParamField>::LOOKUP, stringify!($field))),*];
				let fields: Vec<&str> = fields.iter().filter(|(lookup, _)| !lookup).map(|(_, field)| *field).collect();
				if fields.is_empty() {
					return String::new();
				}
				let locals = fields.iter().map(|field| format!("\tlet {field} = parameters.{field};\n")).collect::<String>();
				format!("\tlet parameters = params();\n{locals}")
			}

			#[allow(unused_mut, unused_variables, unused_assignments)]
			fn wgsl_lookups(binding: usize) -> String {
				let mut binding = binding;
				let mut source = String::new();
				$(if <$ty as $crate::raster_kernel::GpuParamField>::LOOKUP {
					source += &<$ty as $crate::raster_kernel::GpuParamField>::wgsl_lookup(stringify!($field), binding);
					binding += 1;
				})*
				source
			}

			#[allow(unused_mut)]
			fn lookups(&self) -> Vec<$crate::raster_kernel::RasterKernelLookup<'_>> {
				let mut lookups = Vec::new();
				$(<$ty as $crate::raster_kernel::GpuParamField>::lookup(&self.$field, &mut lookups);)*
				lookups
			}
		}
	};
}

/// Implements [`GpuParamField`] for enums converting into the `u32` or `i32` primitive `$repr`, such as through `IntoPrimitive`.
#[macro_export]
macro_rules! gpu_param_enum {
	($repr:ident: $($ty:ty),* $(,)?) => {
		$(
			impl $crate::raster_kernel::GpuParamField for $ty {
				const SLOTS: usize = 1;
				fn wgsl_type() -> String {
					<$repr as $crate::raster_kernel::GpuParamField>::wgsl_type()
				}

				fn write(&self, slots: &mut Vec<u32>) {
					<$repr as $crate::raster_kernel::GpuParamField>::write(&$repr::from(*self), slots);
				}

				fn wgsl_read(slot: usize) -> String {
					<$repr as $crate::raster_kernel::GpuParamField>::wgsl_read(slot)
				}
			}
		)*
	};
}

/// The [`GpuParams`] a kernel passes to its shader, which is the kernel itself unless it implements this to flatten richer data.
pub trait ToGpuParams {
	type Params: GpuParams;
	fn to_params(&self) -> Self::Params;
}

impl<T: GpuParams + Clone> ToGpuParams for T {
	type Params = T;
	fn to_params(&self) -> T {
		self.clone()
	}
}

/// A per-pixel color mapping, run by `map_cpu` on the CPU and by its WGSL on the GPU, which must agree.
///
/// Its WGSL function is `fn map_color(color: vec4<f32>) -> vec4<f32>`.
pub trait MapColor: ToGpuParams + 'static {
	fn map_cpu(&self, color: Color) -> Color;
	fn wgsl() -> Wgsl;
}

/// A GPU-only kernel computing each output pixel from `INPUTS` input rasters, which a type implements instead of [`MapColor`], not alongside it.
///
/// Its WGSL function is `fn kernel(coord: vec2<i32>) -> vec4<f32>`, which may call `output_size()` and, for each input `i`,
/// `input_i_size()`, `input_i_load(coord)` reading the input's own texel clamped to its edge,
/// and `input_i_at(coord)` sampling the input bilinearly where the output pixel's center lies, transparent beyond the input.
pub trait RasterKernel: ToGpuParams + 'static {
	const INPUTS: usize;
	/// Whether the inputs are bound filterable, adding `input_i_sample(texel)` sampling the input bilinearly at texel coordinates, whose texel centers lie at +0.5, clamped to its edge.
	/// Every input must then be in a filterable format such as [`FILTERABLE_FORMAT`], which [`RASTER_FORMAT`] isn't.
	const FILTERED: bool = false;
	fn wgsl() -> Wgsl;
}

/// The format of intermediate rasters a [`RasterKernel::FILTERED`] kernel reads, half-float precision but filterable on WebGL2 unlike [`RASTER_FORMAT`].
pub const FILTERABLE_FORMAT: WgpuTextureFormat = WgpuTextureFormat::Rgba16Float;

fn shader_module<Params: GpuParams>(signature: &str, wgsl: Wgsl, inputs: usize) -> String {
	let function = match wgsl {
		Wgsl::Body(body) => format!("{signature} {{\n{}{body}\n}}", Params::wgsl_locals()),
		Wgsl::Function(source) => source,
	};
	format!("{}\n{}\n{}\n{function}", include_str!("wgsl/prelude.wgsl"), Params::wgsl(), Params::wgsl_lookups(2 + inputs))
}

fn map_color_module<K: MapColor>() -> String {
	let module = shader_module::<K::Params>("fn map_color(color: vec4<f32>) -> vec4<f32>", K::wgsl(), 1);
	format!("{module}\nfn kernel(coord: vec2<i32>) -> vec4<f32> {{\n\treturn map_color(input_0_load(coord));\n}}\n")
}

fn raster_kernel_module<K: RasterKernel>() -> String {
	shader_module::<K::Params>("fn kernel(coord: vec2<i32>) -> vec4<f32>", K::wgsl(), K::INPUTS)
}

fn run<K: ToGpuParams + 'static>(
	kernel: &K,
	wgsl: fn() -> String,
	filtered: bool,
	inputs: Vec<RasterKernelInput<'_>>,
	output_size: UVec2,
	output_format: WgpuTextureFormat,
	pipeline: &WgpuPipelineCache,
) -> Result<Raster<GPU>, NodeError> {
	let gpu_params = kernel.to_params();
	let mut params = Vec::new();
	gpu_params.write(&mut params);
	let args = RasterKernelArgs {
		kernel: TypeId::of::<K>(),
		label: type_name::<K>(),
		wgsl,
		params,
		inputs,
		lookups: gpu_params.lookups(),
		filtered,
		output_size,
		output_format,
	};
	let executor = pipeline.executor().ok_or_else(|| NodeError::new("A GPU is required to process a GPU raster"))?;
	let limit = executor.context().device.limits().max_texture_dimension_2d;
	if output_size.max_element() > limit {
		return Err(ImageError::TooLarge {
			width: output_size.x,
			height: output_size.y,
		}
		.into());
	}
	Ok(pipeline.run::<RasterKernelRuntime>(&args)?)
}

fn raster_format(pipeline: &WgpuPipelineCache) -> WgpuTextureFormat {
	pipeline.executor().map_or(RASTER_FORMAT, WgpuExecutor::raster_format)
}

/// Runs `kernel` on its inputs, each placed by the affine from output pixel coordinates to its texel coordinates, into a raster of `output_size`.
pub fn run_raster_kernel<K: RasterKernel>(kernel: &K, inputs: Vec<RasterKernelInput<'_>>, output_size: UVec2, pipeline: &WgpuPipelineCache) -> Result<Raster<GPU>, NodeError> {
	run_raster_kernel_with_format(kernel, inputs, output_size, raster_format(pipeline), pipeline)
}

/// Runs `kernel` like [`run_raster_kernel`] into a texture of `output_format` instead of the executor's raster format, such as a [`FILTERABLE_FORMAT`] intermediate a [`RasterKernel::FILTERED`] kernel can read.
pub fn run_raster_kernel_with_format<K: RasterKernel>(
	kernel: &K,
	inputs: Vec<RasterKernelInput<'_>>,
	output_size: UVec2,
	output_format: WgpuTextureFormat,
	pipeline: &WgpuPipelineCache,
) -> Result<Raster<GPU>, NodeError> {
	assert_eq!(inputs.len(), K::INPUTS, "{} takes {} inputs", type_name::<K>(), K::INPUTS);
	run(kernel, raster_kernel_module::<K>, K::FILTERED, inputs, output_size, output_format, pipeline)
}

pub trait Adjust: Sized {
	fn adjust<K: MapColor>(self, kernel: &K, pipeline: &WgpuPipelineCache) -> Result<Self, NodeError>;
}

impl Adjust for Color {
	fn adjust<K: MapColor>(self, kernel: &K, _: &WgpuPipelineCache) -> Result<Self, NodeError> {
		Ok(kernel.map_cpu(self))
	}
}

impl Adjust for Gradient {
	fn adjust<K: MapColor>(self, kernel: &K, _: &WgpuPipelineCache) -> Result<Self, NodeError> {
		Ok(self.map_colors(|color| kernel.map_cpu(*color)))
	}
}

impl Adjust for Raster<CPU> {
	fn adjust<K: MapColor>(mut self, kernel: &K, _: &WgpuPipelineCache) -> Result<Self, NodeError> {
		for color in self.data_mut().data.iter_mut() {
			*color = kernel.map_cpu(*color);
		}
		Ok(self)
	}
}

impl Adjust for Raster<GPU> {
	fn adjust<K: MapColor>(self, kernel: &K, pipeline: &WgpuPipelineCache) -> Result<Self, NodeError> {
		if self.is_empty() {
			return Ok(self);
		}

		let size = self.size();
		let input = RasterKernelInput {
			raster: &self,
			output_to_texel: DAffine2::IDENTITY,
		};
		run(kernel, map_color_module::<K>, false, vec![input], size, raster_format(pipeline), pipeline)
	}
}

impl<T: Adjust> Adjust for Item<T> {
	fn adjust<K: MapColor>(self, kernel: &K, pipeline: &WgpuPipelineCache) -> Result<Self, NodeError> {
		let (element, attributes) = self.into_parts();
		Ok(Item::from_parts(element.adjust(kernel, pipeline)?, attributes))
	}
}

#[node_macro::node(category(""), inject_scope)]
fn raster_kernel_pipeline(
	_: impl Ctx,
	#[scope(ProtoNodeIdentifier::new("graphene_std::platform_application_io::TryWgpuExecutorNode"))] executor: Item<Option<WgpuExecutor>>,
	#[data] pipeline: WgpuPipelineCache,
) -> Item<WgpuPipelineCache> {
	if let Some(executor) = executor.into_element() {
		executor.pipeline_init::<RasterKernelRuntime>(pipeline);
	}
	Item::new_from_element(pipeline.clone())
}

/// The most pixels along each side of a [`union_frame`], which coarsens its grid to fit, within the texture size most WebGL2 devices support.
pub const MAX_FRAME_SIDE: u32 = 8192;

/// The output raster of a kernel over placed inputs.
pub struct KernelFrame {
	pub size: UVec2,
	/// The output item's transform, mapping the unit square onto the frame in document space.
	pub transform: DAffine2,
	/// For each input, the affine from output pixel coordinates to its texel coordinates.
	pub output_to_texel: Vec<DAffine2>,
}

impl KernelFrame {
	/// The frame of `size` pixels placed by `transform`, over `inputs` given as each raster's size and its invertible transform.
	pub fn new(size: UVec2, transform: DAffine2, inputs: &[(UVec2, DAffine2)]) -> Self {
		let output_to_document = transform * DAffine2::from_scale(1. / size.as_dvec2());
		let output_to_texel = inputs
			.iter()
			.map(|(input_size, input_transform)| DAffine2::from_scale(input_size.as_dvec2()) * input_transform.inverse() * output_to_document)
			.collect();
		Self { size, transform, output_to_texel }
	}

	/// The kernel inputs placing `rasters`, in the order of the frame's inputs.
	pub fn kernel_inputs<'a>(&self, rasters: &[&'a Raster<GPU>]) -> Vec<RasterKernelInput<'a>> {
		rasters
			.iter()
			.zip(&self.output_to_texel)
			.map(|(raster, output_to_texel)| RasterKernelInput {
				raster,
				output_to_texel: *output_to_texel,
			})
			.collect()
	}
}

/// A raster item's size in pixels and its `ATTR_TRANSFORM`, as [`union_frame`] and [`KernelFrame::new`] take each input.
pub fn raster_placement(raster: &Item<Raster<GPU>>) -> (UVec2, DAffine2) {
	(raster.element().size(), raster.attribute_cloned_or_default(ATTR_TRANSFORM))
}

/// The frame covering the union of `inputs`, each a raster's size and transform, on the finest of their pixel grids.
///
/// The grid's axes follow the first input's, with the smallest pixel step along each that any input has, anchored at the first input's origin.
/// Each side is capped at [`MAX_FRAME_SIDE`] pixels. Empty inputs are ignored. `None` without a non-empty input or with a non-invertible one.
pub fn union_frame(inputs: &[(UVec2, DAffine2)]) -> Option<KernelFrame> {
	let placed = inputs.iter().copied().filter(|(size, _)| size.x > 0 && size.y > 0).collect::<Vec<_>>();
	let first = placed.first()?.1;
	if placed.iter().any(|(_, transform)| transform.matrix2.determinant() == 0.) {
		return None;
	}

	let axes = DMat2::from_cols(first.matrix2.x_axis.normalize(), first.matrix2.y_axis.normalize());
	let to_axes = axes.inverse();

	let corners = placed
		.iter()
		.flat_map(|(_, transform)| [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE].map(|corner| to_axes * transform.transform_point2(corner)));
	let (min, max) = corners.fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(min, max), corner| (min.min(corner), max.max(corner)));

	let step = placed
		.iter()
		.map(|(size, transform)| {
			let to_texels = DMat2::from_diagonal(size.as_dvec2()) * transform.matrix2.inverse() * axes;
			1. / DVec2::new(to_texels.x_axis.length(), to_texels.y_axis.length())
		})
		.fold(DVec2::INFINITY, DVec2::min);

	let anchor = to_axes * first.translation;
	let start = anchor + ((min - anchor) / step + 1e-6).floor() * step;
	let end = anchor + ((max - anchor) / step - 1e-6).ceil() * step;
	let cells = ((end - start) / step).round().max(DVec2::ONE);

	let max_side = DVec2::splat(MAX_FRAME_SIDE as f64);
	let oversized = cells.cmpgt(max_side);
	let start = DVec2::select(oversized, min, start);
	let step = DVec2::select(oversized, (max - min) / max_side, step);
	let cells = cells.min(max_side);

	let transform = DAffine2::from_mat2_translation(axes * DMat2::from_diagonal(cells * step), axes * start);
	Some(KernelFrame::new(cells.as_uvec2(), transform, inputs))
}

/// A raster whose pixels can be read on the CPU, downloading them when they're on the GPU.
pub trait ReadPixels: Sized {
	fn read_pixels(self, pipeline: &WgpuPipelineCache) -> impl Future<Output = Result<Raster<CPU>, NodeError>> + Send;
}

impl ReadPixels for Raster<CPU> {
	async fn read_pixels(self, _: &WgpuPipelineCache) -> Result<Raster<CPU>, NodeError> {
		Ok(self)
	}
}

impl ReadPixels for Raster<GPU> {
	fn read_pixels(self, pipeline: &WgpuPipelineCache) -> impl Future<Output = Result<Raster<CPU>, NodeError>> + Send {
		let executor = pipeline.executor().cloned();
		async move {
			if self.is_empty() {
				return Ok(Raster::default());
			}

			let executor = executor.ok_or_else(|| NodeError::new("A GPU is required to read a GPU raster"))?;
			Ok(self.convert(Footprint::DEFAULT, executor).await)
		}
	}
}
