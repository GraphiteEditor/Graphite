use crate::shaders::buffer_struct::{BufferStruct, BufferStructIdentity};
use bytemuck::Pod;
use core::marker::PhantomData;
use core::num::Wrapping;
use spirv_std::arch::IndexUnchecked;

macro_rules! identity {
	($t:ty) => {
		impl BufferStructIdentity for $t {}
	};
}

identity!(());
identity!(u8);
identity!(u16);
identity!(u32);
identity!(u64);
identity!(u128);
identity!(usize);
identity!(i8);
identity!(i16);
identity!(i32);
identity!(i64);
identity!(i128);
identity!(isize);
identity!(f32);
identity!(f64);

identity!(spirv_std::arch::SubgroupMask);
identity!(spirv_std::memory::Semantics);
identity!(spirv_std::ray_tracing::RayFlags);
identity!(spirv_std::indirect_command::DrawIndirectCommand);
identity!(spirv_std::indirect_command::DrawIndexedIndirectCommand);
identity!(spirv_std::indirect_command::DispatchIndirectCommand);
identity!(spirv_std::indirect_command::DrawMeshTasksIndirectCommandEXT);
identity!(spirv_std::indirect_command::TraceRaysIndirectCommandKHR);
// not pod
// identity!(spirv_std::indirect_command::TraceRaysIndirectCommand2KHR);

unsafe impl BufferStruct for bool {
	type Buffer = u32;

	#[inline]
	fn write(from: Self) -> Self::Buffer {
		from as u32
	}

	#[inline]
	fn read(from: Self::Buffer) -> Self {
		from != 0
	}
}

unsafe impl<T: BufferStruct> BufferStruct for Wrapping<T>
where
	// unfortunately has to be Pod, even though AnyBitPattern would be sufficient,
	// due to bytemuck doing `impl<T: Pod> AnyBitPattern for T {}`
	// see https://github.com/Lokathor/bytemuck/issues/164
	T::Buffer: Pod,
{
	type Buffer = Wrapping<T::Buffer>;

	#[inline]
	fn write(from: Self) -> Self::Buffer {
		Wrapping(T::write(from.0))
	}

	#[inline]
	fn read(from: Self::Buffer) -> Self {
		Wrapping(T::read(from.0))
	}
}

unsafe impl<T: BufferStruct + 'static> BufferStruct for PhantomData<T> {
	type Buffer = PhantomData<T>;

	#[inline]
	fn write(_: Self) -> Self::Buffer {
		PhantomData {}
	}

	#[inline]
	fn read(_: Self::Buffer) -> Self {
		PhantomData {}
	}
}

/// Potential problem: you can't impl this for an array of BufferStruct, as it'll conflict with this impl due to the
/// blanket impl on all BufferStructPlain types.
unsafe impl<T: BufferStruct, const N: usize> BufferStruct for [T; N]
where
	// rust-gpu does not like `[T; N].map()` nor `core::array::from_fn()` nor transmuting arrays with a const generic
	// length, so for now we need to require T: Default and T::Transfer: Default for all arrays.
	T: Default,
	// unfortunately has to be Pod, even though AnyBitPattern would be sufficient,
	// due to bytemuck doing `impl<T: Pod> AnyBitPattern for T {}`
	// see https://github.com/Lokathor/bytemuck/issues/164
	T::Buffer: Pod + Default,
{
	type Buffer = [T::Buffer; N];

	#[inline]
	fn write(from: Self) -> Self::Buffer {
		unsafe {
			let mut ret = [T::Buffer::default(); N];
			for i in 0..N {
				*ret.index_unchecked_mut(i) = T::write(*from.index_unchecked(i));
			}
			ret
		}
	}

	#[inline]
	fn read(from: Self::Buffer) -> Self {
		unsafe {
			let mut ret = [T::default(); N];
			for i in 0..N {
				*ret.index_unchecked_mut(i) = T::read(*from.index_unchecked(i));
			}
			ret
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn roundtrip_bool() {
		for x in [false, true] {
			assert_eq!(x, <bool as BufferStruct>::read(<bool as BufferStruct>::write(x)));
		}
	}
}
