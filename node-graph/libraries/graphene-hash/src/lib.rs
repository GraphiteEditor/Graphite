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
	core::time::Duration,
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

/// rustc-hash's polynomial hash with the state pinned to u64, so keys match across native and wasm targets.
#[derive(Clone, Default)]
pub struct FxHasher64 {
	hash: u64,
}

const K: u64 = 0xf1357aea2e62a9c5;
const SEED1: u64 = 0x243f6a8885a308d3;
const SEED2: u64 = 0x13198a2e03707344;
const PREVENT_TRIVIAL_ZERO_COLLAPSE: u64 = 0xa4093822299f31d0;

impl FxHasher64 {
	pub const fn new() -> Self {
		Self { hash: 0 }
	}

	#[inline]
	fn add_to_hash(&mut self, i: u64) {
		self.hash = self.hash.wrapping_add(i).wrapping_mul(K);
	}
}

impl core::hash::Hasher for FxHasher64 {
	#[inline]
	fn write(&mut self, bytes: &[u8]) {
		self.add_to_hash(hash_bytes(bytes));
	}

	#[inline]
	fn write_u8(&mut self, i: u8) {
		self.add_to_hash(i as u64);
	}

	#[inline]
	fn write_u16(&mut self, i: u16) {
		self.add_to_hash(i as u64);
	}

	#[inline]
	fn write_u32(&mut self, i: u32) {
		self.add_to_hash(i as u64);
	}

	#[inline]
	fn write_u64(&mut self, i: u64) {
		self.add_to_hash(i);
	}

	#[inline]
	fn write_u128(&mut self, i: u128) {
		self.add_to_hash(i as u64);
		self.add_to_hash((i >> 64) as u64);
	}

	#[inline]
	fn write_usize(&mut self, i: usize) {
		self.add_to_hash(i as u64);
	}

	#[inline]
	fn finish(&self) -> u64 {
		self.hash.rotate_left(26)
	}
}

#[inline]
fn multiply_mix(x: u64, y: u64) -> u64 {
	let full = (x as u128) * (y as u128);
	(full as u64) ^ ((full >> 64) as u64)
}

#[inline]
fn hash_bytes(bytes: &[u8]) -> u64 {
	let len = bytes.len();
	let mut s0 = SEED1;
	let mut s1 = SEED2;

	if len <= 16 {
		if len >= 8 {
			s0 ^= u64::from_le_bytes(bytes[0..8].try_into().unwrap());
			s1 ^= u64::from_le_bytes(bytes[len - 8..].try_into().unwrap());
		} else if len >= 4 {
			s0 ^= u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as u64;
			s1 ^= u32::from_le_bytes(bytes[len - 4..].try_into().unwrap()) as u64;
		} else if len > 0 {
			let lo = bytes[0];
			let mid = bytes[len / 2];
			let hi = bytes[len - 1];
			s0 ^= lo as u64;
			s1 ^= ((hi as u64) << 8) | mid as u64;
		}
	} else {
		let mut off = 0;
		while off < len - 16 {
			let x = u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
			let y = u64::from_le_bytes(bytes[off + 8..off + 16].try_into().unwrap());
			let t = multiply_mix(s0 ^ x, PREVENT_TRIVIAL_ZERO_COLLAPSE ^ y);
			s0 = s1;
			s1 = t;
			off += 16;
		}

		let suffix = &bytes[len - 16..];
		s0 ^= u64::from_le_bytes(suffix[0..8].try_into().unwrap());
		s1 ^= u64::from_le_bytes(suffix[8..16].try_into().unwrap());
	}

	multiply_mix(s0, s1) ^ (len as u64)
}
