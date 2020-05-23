use crate::gui_attributes::*;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct WindowUniform {
	pub dimensions: Dimensions<u32>,
}

impl WindowUniform {
	pub fn new(width: u32, height: u32) -> Self {
		Self {
			dimensions: Dimensions::new(width, height),
		}
	}
}

unsafe impl bytemuck::Zeroable for WindowUniform {}
unsafe impl bytemuck::Pod for WindowUniform {}
