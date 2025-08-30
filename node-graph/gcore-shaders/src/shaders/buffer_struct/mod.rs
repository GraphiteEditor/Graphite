//! I (@firestar99) copied this entire mod from one of my projects, as I haven't uploaded that lib to crates. Hopefully
//! rust-gpu improves and this entire thing becomes unnecessary in the future.
//!
//! https://github.com/Firestar99/nanite-at-home/tree/008dac8df656959c71efeddd2d3ddabcb801771c/rust-gpu-bindless/crates/buffer-content

use bytemuck::Pod;

mod glam;
mod primitive;

/// A BufferStruct is a "parallel representation" of the original struct with some fundamental types remapped. This
/// struct hierarchy represents how data is stored in GPU Buffers, where all types must be [`Pod`] to allow
/// transmuting them to `&[u8]` with [`bytemuck`].
///
/// Notable type remappings (original: buffer):
/// * bool: u32 of 0 or 1
/// * any repr(u32) enum: u32 with remapping via [`num_enum`]
///
/// By adding `#[derive(ShaderStruct)]` to your struct (or enum), a parallel `{name}Buffer` struct is created with all
/// the members of the original struct, but with their types using the associated remapped types as specified by this
/// trait.
///
/// # Origin
/// I (@firestar99) copied this entire mod from my [Nanite-at-home] project, specifically the [buffer-content] crate
/// and the [buffer_struct] proc macro. The variant here has quite some modifications, to both cleaned up some of the
/// mistakes my implementation has and to customize it a bit for graphite.
///
/// Hopefully rust-gpu improves to the point where this remapping becomes unnecessary.
///
/// [Nanite-at-home]: https://github.com/Firestar99/nanite-at-home
/// [buffer-content]: https://github.com/Firestar99/nanite-at-home/tree/008dac8df656959c71efeddd2d3ddabcb801771c/rust-gpu-bindless/crates/buffer-content
/// [buffer_struct]: https://github.com/Firestar99/nanite-at-home/blob/008dac8df656959c71efeddd2d3ddabcb801771c/rust-gpu-bindless/crates/macros/src/buffer_struct.rs
///
/// # Safety
/// The associated type Transfer must be the same on all targets. Writing followed by reading back a value must result
/// in the same value.
pub unsafe trait BufferStruct: Copy + Send + Sync + 'static {
	type Buffer: Pod + Send + Sync;

	fn write(from: Self) -> Self::Buffer;

	fn read(from: Self::Buffer) -> Self;
}

/// Trait marking all [`BufferStruct`] whose read and write methods are identity. While [`BufferStruct`] only
/// requires `t == read(write(t))`, this trait additionally requires `t == read(t) == write(t)`. As this removes the
/// conversion requirement for writing to or reading from a buffer, one can acquire slices from buffers created of these
/// types.
///
/// Implementing this type is completely safe due to the [`Pod`] requirement.
pub trait BufferStructIdentity: Pod + Send + Sync {}

unsafe impl<T: BufferStructIdentity> BufferStruct for T {
	type Buffer = Self;

	fn write(from: Self) -> Self::Buffer {
		from
	}

	fn read(from: Self::Buffer) -> Self {
		from
	}
}
