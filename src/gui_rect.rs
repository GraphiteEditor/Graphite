pub struct GuiRect {
	pub corners: Corners<(f32, f32)>,
	pub corners_radius: Corners<f32>,
	pub sides_inset: Sides<f32>,
	pub border: f32,
	pub border_color: wgpu::Color,
	pub fill_color: wgpu::Color,
	pub fill_texture: Option<wgpu::Texture>,
}

pub struct Corners<T> {
	pub top_left: T,
	pub top_right: T,
	pub bottom_right: T,
	pub bottom_left: T,
}

pub struct Sides<T> {
	pub top: T,
	pub right: T,
	pub bottom: T,
	pub left: T,
}
