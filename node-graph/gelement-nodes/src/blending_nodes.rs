use graphene_core::GraphicGroupTable;
use graphene_core::blending::BlendMode;
use graphene_core::color::Color;
use graphene_core::context::Ctx;
use graphene_core::raster_types::{CPU, RasterDataTable};
use graphene_core::registry::types::Percentage;
use graphene_core::vector::VectorDataTable;

pub(super) trait MultiplyAlpha {
	fn multiply_alpha(&mut self, factor: f64);
}

impl MultiplyAlpha for Color {
	fn multiply_alpha(&mut self, factor: f64) {
		*self = Color::from_rgbaf32_unchecked(self.r(), self.g(), self.b(), (self.a() * factor as f32).clamp(0., 1.))
	}
}
impl MultiplyAlpha for VectorDataTable {
	fn multiply_alpha(&mut self, factor: f64) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.opacity *= factor as f32;
		}
	}
}
impl MultiplyAlpha for GraphicGroupTable {
	fn multiply_alpha(&mut self, factor: f64) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.opacity *= factor as f32;
		}
	}
}
impl MultiplyAlpha for RasterDataTable<CPU> {
	fn multiply_alpha(&mut self, factor: f64) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.opacity *= factor as f32;
		}
	}
}

pub(super) trait MultiplyFill {
	fn multiply_fill(&mut self, factor: f64);
}
impl MultiplyFill for Color {
	fn multiply_fill(&mut self, factor: f64) {
		*self = Color::from_rgbaf32_unchecked(self.r(), self.g(), self.b(), (self.a() * factor as f32).clamp(0., 1.))
	}
}
impl MultiplyFill for VectorDataTable {
	fn multiply_fill(&mut self, factor: f64) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.fill *= factor as f32;
		}
	}
}
impl MultiplyFill for GraphicGroupTable {
	fn multiply_fill(&mut self, factor: f64) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.fill *= factor as f32;
		}
	}
}
impl MultiplyFill for RasterDataTable<CPU> {
	fn multiply_fill(&mut self, factor: f64) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.fill *= factor as f32;
		}
	}
}

trait SetBlendMode {
	fn set_blend_mode(&mut self, blend_mode: BlendMode);
}

impl SetBlendMode for VectorDataTable {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.blend_mode = blend_mode;
		}
	}
}
impl SetBlendMode for GraphicGroupTable {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.blend_mode = blend_mode;
		}
	}
}
impl SetBlendMode for RasterDataTable<CPU> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.blend_mode = blend_mode;
		}
	}
}

trait SetClip {
	fn set_clip(&mut self, clip: bool);
}

impl SetClip for VectorDataTable {
	fn set_clip(&mut self, clip: bool) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.clip = clip;
		}
	}
}
impl SetClip for GraphicGroupTable {
	fn set_clip(&mut self, clip: bool) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.clip = clip;
		}
	}
}
impl SetClip for RasterDataTable<CPU> {
	fn set_clip(&mut self, clip: bool) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.clip = clip;
		}
	}
}

#[node_macro::node(category("Style"))]
fn blend_mode<T: SetBlendMode>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
		VectorDataTable,
		RasterDataTable<CPU>,
	)]
	mut value: T,
	blend_mode: BlendMode,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its row in its parent table or Instance<T>) rather than applying to each row in its own table, which produces the undesired result
	value.set_blend_mode(blend_mode);
	value
}

#[node_macro::node(category("Style"))]
fn opacity<T: MultiplyAlpha>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
		VectorDataTable,
		RasterDataTable<CPU>,
	)]
	mut value: T,
	#[default(100.)] opacity: Percentage,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its row in its parent table or Instance<T>) rather than applying to each row in its own table, which produces the undesired result
	value.multiply_alpha(opacity / 100.);
	value
}

#[node_macro::node(category("Style"))]
fn blending<T: SetBlendMode + MultiplyAlpha + MultiplyFill + SetClip>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
		VectorDataTable,
		RasterDataTable<CPU>,
	)]
	mut value: T,
	blend_mode: BlendMode,
	#[default(100.)] opacity: Percentage,
	#[default(100.)] fill: Percentage,
	#[default(false)] clip: bool,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its row in its parent table or Instance<T>) rather than applying to each row in its own table, which produces the undesired result
	value.set_blend_mode(blend_mode);
	value.multiply_alpha(opacity / 100.);
	value.multiply_fill(fill / 100.);
	value.set_clip(clip);
	value
}
