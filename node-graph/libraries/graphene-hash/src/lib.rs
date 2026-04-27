#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "derive")]
pub use graphene_hash_derive::CacheHash;

pub trait CacheHash {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H);
}

/// Wrapper that implements `std::hash::Hash` by delegating to `CacheHash`.
///
/// Use this to store `CacheHash` types in `HashMap`/`HashSet` keys,
/// making it explicit that float fields are hashed via bit patterns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheHashWrapper<T>(pub T);

impl<T: CacheHash> core::hash::Hash for CacheHashWrapper<T> {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.0.cache_hash(state);
	}
}

impl<T: CacheHash> CacheHash for core::ops::RangeInclusive<T> {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.start().cache_hash(state);
		self.end().cache_hash(state);
	}
}

impl<T> core::ops::Deref for CacheHashWrapper<T> {
	type Target = T;
	fn deref(&self) -> &T {
		&self.0
	}
}

// Bulk impl for types that already implement std::hash::Hash — delegates directly.
#[macro_export]
macro_rules! impl_via_hash {
	($($t:ty),* $(,)?) => {
		$(
			impl $crate::CacheHash for $t {
				#[inline]
				fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
					core::hash::Hash::hash(self, state);
				}
			}
		)*
	};
}

impl_via_hash! {
	bool, char,
	u8, u16, u32, u64, u128, usize,
	i8, i16, i32, i64, i128, isize,
	// glam integer vector types have Hash
	glam::UVec2, glam::UVec3, glam::UVec4,
	glam::IVec2, glam::IVec3, glam::IVec4,
	glam::I64Vec2, glam::I64Vec3, glam::I64Vec4,
	glam::U64Vec2, glam::U64Vec3, glam::U64Vec4,
	glam::BVec2, glam::BVec3, glam::BVec4,
}

#[cfg(feature = "std")]
impl_via_hash! {
	String,
}

impl<'a> CacheHash for std::borrow::Cow<'a, str> {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(self, state);
	}
}

impl CacheHash for str {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(self, state);
	}
}

impl CacheHash for () {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, _state: &mut H) {}
}

// f32 and f64: hash via bit pattern so NaN is handled deterministically.
impl CacheHash for f32 {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(&self.to_bits(), state);
	}
}

impl CacheHash for f64 {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(&self.to_bits(), state);
	}
}

// glam float vector/matrix types: hash each component via to_bits().
macro_rules! impl_glam_array {
	($($t:ty),* $(,)?) => {
		$(
			impl CacheHash for $t {
				#[inline]
				fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
					for v in self.to_array() {
						CacheHash::cache_hash(&v, state);
					}
				}
			}
		)*
	};
}

macro_rules! impl_glam_cols {
	($($t:ty),* $(,)?) => {
		$(
			impl CacheHash for $t {
				#[inline]
				fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
					for v in self.to_cols_array() {
						CacheHash::cache_hash(&v, state);
					}
				}
			}
		)*
	};
}

impl_glam_array! {
	glam::Vec2, glam::Vec3, glam::Vec3A, glam::Vec4,
	glam::DVec2, glam::DVec3, glam::DVec4,
}

impl_glam_cols! {
	glam::Mat2, glam::Mat3, glam::Mat3A, glam::Mat4,
	glam::DMat2, glam::DMat3, glam::DMat4,
	glam::Affine2, glam::Affine3A,
	glam::DAffine2, glam::DAffine3,
}

// Quat / DQuat — to_array gives [x, y, z, w] as floats
impl_glam_array! {
	glam::Quat, glam::DQuat,
}

// Generic container impls.
impl<T: CacheHash> CacheHash for Option<T> {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		match self {
			None => core::hash::Hash::hash(&0u8, state),
			Some(v) => {
				core::hash::Hash::hash(&1u8, state);
				v.cache_hash(state);
			}
		}
	}
}

impl<T: CacheHash> CacheHash for [T] {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(&self.len(), state);
		for item in self {
			item.cache_hash(state);
		}
	}
}

impl<T: CacheHash, const N: usize> CacheHash for [T; N] {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		for item in self {
			item.cache_hash(state);
		}
	}
}

#[cfg(feature = "std")]
impl<T: CacheHash> CacheHash for Vec<T> {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.as_slice().cache_hash(state);
	}
}

#[cfg(feature = "std")]
impl<T: CacheHash + ?Sized> CacheHash for Box<T> {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		(**self).cache_hash(state);
	}
}

#[cfg(feature = "std")]
impl<T: CacheHash + ?Sized> CacheHash for std::sync::Arc<T> {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		(**self).cache_hash(state);
	}
}

impl<T: CacheHash + ?Sized> CacheHash for &T {
	#[inline]
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		(**self).cache_hash(state);
	}
}

// Tuple impls.
macro_rules! impl_tuple {
	($($T:ident),+) => {
		impl<$($T: CacheHash),+> CacheHash for ($($T,)+) {
			#[inline]
			#[allow(non_snake_case)]
			fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
				let ($($T,)+) = self;
				$($T.cache_hash(state);)+
			}
		}
	};
}

impl_tuple!(A, B);
impl_tuple!(A, B, C);
impl_tuple!(A, B, C, D);
impl_tuple!(A, B, C, D, E);
impl_tuple!(A, B, C, D, E, F);
