use core_types::color::Color;

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

		impl $crate::GpuParamField for $name {
			const SLOTS: usize = 0 $(+ <$ty as $crate::GpuParamField>::SLOTS)*;
			fn wgsl_type() -> String {
				stringify!($name).into()
			}

			#[allow(unused_variables)]
			fn write(&self, slots: &mut Vec<u32>) {
				$(<$ty as $crate::GpuParamField>::write(&self.$field, slots);)*
			}

			#[allow(unused_mut, unused_variables, unused_assignments)]
			fn wgsl_read(slot: usize) -> String {
				let mut slot = slot;
				let reads: Vec<(bool, String)> = vec![$({
					let read = <$ty as $crate::GpuParamField>::wgsl_read(slot);
					slot += <$ty as $crate::GpuParamField>::SLOTS;
					(<$ty as $crate::GpuParamField>::LOOKUP, read)
				}),*];
				let reads: Vec<String> = reads.into_iter().filter(|(lookup, _)| !lookup).map(|(_, read)| read).collect();
				format!("{}({})", stringify!($name), reads.join(", "))
			}

			#[allow(unused_variables)]
			fn wgsl_declarations(declarations: &mut Vec<String>) {
				$(<$ty as $crate::GpuParamField>::wgsl_declarations(declarations);)*
				let fields: Vec<(bool, String)> = vec![$((
					<$ty as $crate::GpuParamField>::LOOKUP,
					format!("{}: {}", stringify!($field), <$ty as $crate::GpuParamField>::wgsl_type())
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

		impl $crate::GpuParams for $name {
			fn write(&self, slots: &mut Vec<u32>) {
				<Self as $crate::GpuParamField>::write(self, slots);
			}

			fn wgsl() -> String {
				let mut declarations = Vec::new();
				<Self as $crate::GpuParamField>::wgsl_declarations(&mut declarations);
				if declarations.is_empty() {
					return String::new();
				}
				format!(
					"{}\nalias Params = {};\n\nfn params() -> Params {{\n\treturn {};\n}}\n",
					declarations.join("\n"),
					stringify!($name),
					<Self as $crate::GpuParamField>::wgsl_read(0)
				)
			}

			fn wgsl_locals() -> String {
				let fields: &[(bool, &str)] = &[$((<$ty as $crate::GpuParamField>::LOOKUP, stringify!($field))),*];
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
				$(if <$ty as $crate::GpuParamField>::LOOKUP {
					source += &<$ty as $crate::GpuParamField>::wgsl_lookup(stringify!($field), binding);
					binding += 1;
				})*
				source
			}

			#[allow(unused_mut)]
			fn lookups(&self) -> Vec<$crate::RasterKernelLookup<'_>> {
				let mut lookups = Vec::new();
				$(<$ty as $crate::GpuParamField>::lookup(&self.$field, &mut lookups);)*
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
			impl $crate::GpuParamField for $ty {
				const SLOTS: usize = 1;
				fn wgsl_type() -> String {
					<$repr as $crate::GpuParamField>::wgsl_type()
				}

				fn write(&self, slots: &mut Vec<u32>) {
					<$repr as $crate::GpuParamField>::write(&$repr::from(*self), slots);
				}

				fn wgsl_read(slot: usize) -> String {
					<$repr as $crate::GpuParamField>::wgsl_read(slot)
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
	/// Every input must then be in a filterable format such as `Rgba16Float`, which the executor's raster format isn't.
	const FILTERED: bool = false;
	fn wgsl() -> Wgsl;
}

fn shader_module<Params: GpuParams>(signature: &str, wgsl: Wgsl, inputs: usize) -> String {
	let function = match wgsl {
		Wgsl::Body(body) => format!("{signature} {{\n{}{body}\n}}", Params::wgsl_locals()),
		Wgsl::Function(source) => source,
	};
	format!("{}\n{}\n{}\n{function}", include_str!("wgsl/prelude.wgsl"), Params::wgsl(), Params::wgsl_lookups(2 + inputs))
}

/// The WGSL module running a [`MapColor`] on each texel of its one input.
pub fn map_color_module<K: MapColor>() -> String {
	let module = shader_module::<K::Params>("fn map_color(color: vec4<f32>) -> vec4<f32>", K::wgsl(), 1);
	format!("{module}\nfn kernel(coord: vec2<i32>) -> vec4<f32> {{\n\treturn map_color(input_0_load(coord));\n}}\n")
}

/// The WGSL module of a [`RasterKernel`].
pub fn raster_kernel_module<K: RasterKernel>() -> String {
	shader_module::<K::Params>("fn kernel(coord: vec2<i32>) -> vec4<f32>", K::wgsl(), K::INPUTS)
}

/// A lookup table of a kernel, bound as an `Rgba32Float` texture after its inputs.
pub enum RasterKernelLookup<'a> {
	/// A `width x 1` 2D texture of at most 4096 entries.
	Lut1d(&'a [Color]),
	/// A `size x size x size` 3D texture, its entries ordered with x fastest, then y, then z.
	Lut3d { size: u32, data: &'a [Color] },
}
