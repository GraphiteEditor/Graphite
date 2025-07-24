pub use glam;

pub mod blending;
pub mod choice_type;
pub mod color;
pub mod registry;

pub trait AsU32 {
	fn as_u32(&self) -> u32;
}
impl AsU32 for u32 {
	fn as_u32(&self) -> u32 {
		*self
	}
}
