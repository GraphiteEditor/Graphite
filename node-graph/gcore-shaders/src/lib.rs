#![cfg_attr(not(feature = "std"), no_std)]

pub mod blending;
pub mod choice_type;
pub mod color;
pub mod context;
pub mod registry;
pub mod shaders;

pub use context::Ctx;
pub use glam;

pub trait AsU32 {
	fn as_u32(&self) -> u32;
}
impl AsU32 for u32 {
	fn as_u32(&self) -> u32 {
		*self
	}
}
