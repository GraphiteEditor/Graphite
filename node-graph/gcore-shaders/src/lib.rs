pub use glam;

pub mod blending;
pub mod color;

pub trait AsU32 {
	fn as_u32(&self) -> u32;
}
impl AsU32 for u32 {
	fn as_u32(&self) -> u32 {
		*self
	}
}
