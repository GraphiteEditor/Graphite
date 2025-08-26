//! custom BufferStruct impl for graphite

use crate::color::Color;
use crate::shaders::buffer_struct::BufferStruct;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Zeroable, Pod)]
pub struct OptionalColorBuffer(u32, <Color as BufferStruct>::Buffer);

unsafe impl BufferStruct for Option<Color> {
	type Buffer = OptionalColorBuffer;

	#[inline]
	fn write(from: Self) -> Self::Buffer {
		match from {
			None => OptionalColorBuffer(0, Color::write(Color::default())),
			Some(t) => OptionalColorBuffer(1, Color::write(t)),
		}
	}

	#[inline]
	fn read(from: Self::Buffer) -> Self {
		match from.0 {
			1 => Some(Color::read(from.1)),
			_ => None,
		}
	}
}
