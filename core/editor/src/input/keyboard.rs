pub const NUMBER_OF_KEYS: usize = Key::NumKeys as usize;
// Edit this to specify the storage type used
pub type StorageType = u128;
const STORAGE_SIZE: u32 = std::mem::size_of::<usize>() as u32 * 8 + 2 - std::mem::size_of::<StorageType>().leading_zeros();
const STORAGE_SIZE_BITS: usize = 1 << STORAGE_SIZE;
const KEY_MASK_STORAGE_LENGHT: usize = NUMBER_OF_KEYS + STORAGE_SIZE_BITS - 1 >> STORAGE_SIZE;
pub type Keyboard = KeyStore<KEY_MASK_STORAGE_LENGHT>;

#[derive(Debug, Default)]
pub struct KeyState {
	depressed: bool,
	// time of last press
	// mod keys held down while pressing
	// …
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
	UnknownKey,
	// MouseKeys
	Lmb,
	Rmb,
	Mmb,
	MouseMove,

	// Keyboard keys
	KeyR,
	KeyM,
	KeyE,
	KeyL,
	KeyP,
	KeyV,
	KeyX,
	KeyZ,
	KeyY,
	KeyEnter,
	Key0,
	Key1,
	Key2,
	Key3,
	Key4,
	Key5,
	Key6,
	Key7,
	Key8,
	Key9,
	KeyShift,
	KeyCaps,
	KeyControl,
	KeyAlt,
	KeyEscape,

	// This has to be the last element in the enum.
	NumKeys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyStore<const LENGTH: usize>([StorageType; LENGTH]);

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign};

impl<const LENGTH: usize> KeyStore<LENGTH> {
	#[inline]
	fn convert_index(index: usize) -> (usize, StorageType) {
		let bit = 1 << index & STORAGE_SIZE as StorageType - 1;
		let offset = index >> STORAGE_SIZE;
		(offset, bit)
	}
	pub const fn new() -> Self {
		Self([0; LENGTH])
	}
	pub fn set(&mut self, index: usize) {
		let (offset, bit) = Self::convert_index(index);
		self.0[offset] |= bit;
	}
	pub fn unset(&mut self, index: usize) {
		let (offset, bit) = Self::convert_index(index);
		self.0[offset] &= !bit;
	}
	pub fn toggle(&mut self, index: usize) {
		let (offset, bit) = Self::convert_index(index);
		self.0[offset] ^= bit;
	}
}

impl<const LENGTH: usize> Default for KeyStore<LENGTH> {
	fn default() -> Self {
		Self::new()
	}
}

macro_rules! bit_ops {
	($(($op:ident, $func:ident)),* $(,)?) => {
		$(impl<const LENGTH: usize> $op for KeyStore<LENGTH> {
			type Output = Self;
			fn $func(self, right: Self) -> Self::Output {
				let mut result = Self::new();
				for ((left, right), new) in self.0.iter().zip(right.0.iter()).zip(result.0.iter_mut()) {
					*new = $op::$func(left, right);
				}
				result
			}
		})*
	};
}
macro_rules! bit_ops_assign {
	($(($op:ident, $func:ident)),* $(,)?) => {
		$(impl<const LENGTH: usize> $op for KeyStore<LENGTH> {
			fn $func(&mut self, right: Self)  {
				for (left, right) in self.0.iter_mut().zip(right.0.iter()) {
					$op::$func(left, right);
				}
			}
		})*
	};
}

bit_ops!((BitAnd, bitand), (BitOr, bitor), (BitXor, bitxor));
bit_ops_assign!((BitAndAssign, bitand_assign), (BitOrAssign, bitor_assign), (BitXorAssign, bitxor_assign));
