use glam::{Vec2, Vec4};
use spirv_std::spirv;

/// webgpu NDC is like OpenGL: (-1.0 .. 1.0, -1.0 .. 1.0, 0.0 .. 1.0)
/// https://www.w3.org/TR/webgpu/#coordinate-systems
///
/// So to make a fullscreen triangle around a box at (-1..1):
///
/// ```text
///  3 +
///    |\
///  2 |  \
///    |    \
///  1 +-----+
///    |     |\
///  0 |  0  |  \
///    |     |    \
/// -1 +-----+-----+
///   -1  0  1  2  3
/// ```
const FULLSCREEN_VERTICES: [Vec2; 3] = [Vec2::new(-1., -1.), Vec2::new(-1., 3.), Vec2::new(3., -1.)];

#[spirv(vertex)]
pub fn fullscreen_vertex(#[spirv(vertex_index)] vertex_index: u32, #[spirv(position)] gl_position: &mut Vec4) {
	// broken on edition 2024 branch
	// let vertex = unsafe { *FULLSCREEN_VERTICES.index_unchecked(vertex_index as usize) };
	let vertex = FULLSCREEN_VERTICES[vertex_index as usize];
	*gl_position = Vec4::from((vertex, 0., 1.));
}
