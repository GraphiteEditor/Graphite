use crate::color::Color;

// exact copy of the invert node
// #[node_macro::node(category("Raster: Adjustment"))]
fn invert_copy(
	// _: impl Ctx,
	// #[implementations(
	// 	Color,
	// 	ImageFrameTable<Color>,
	// 	GradientStops,
	// )]
	// mut input: T,
	color: Color,
) -> Color {
	// input.adjust(|color| {
	let color = color.to_gamma_srgb();

	let color = color.map_rgb(|c| color.a() - c);

	color.to_linear_srgb()
	// });
	// input
}

pub mod gpu_invert_shader {
	use crate::color::Color;
	use crate::gpu_invert::invert_copy;
	use glam::{Vec4, Vec4Swizzles};
	use spirv_std::image::sample_with::lod;
	use spirv_std::image::{Image2d, ImageWithMethods};
	use spirv_std::spirv;

	#[spirv(fragment)]
	pub fn gpu_invert_fragment(#[spirv(frag_coord)] frag_coord: Vec4, #[spirv(descriptor_set = 0, binding = 0)] texture: &Image2d, color_out: &mut Vec4) {
		let color = Color::from(texture.fetch_with(frag_coord.xy().as_uvec2(), lod(0)));
		let color = invert_copy(color);
		*color_out = Vec4::from(color);
	}
}
