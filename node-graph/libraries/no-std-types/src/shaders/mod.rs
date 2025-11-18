//! supporting infrastructure for shaders

pub mod buffer_struct;

pub mod __private {
	pub use bytemuck;
	pub use glam;
	pub use num_enum;
	pub use spirv_std;
}
