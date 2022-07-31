use crate::message_prelude::*;

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign};

// TODO: Increase size of type
/// Edit this to specify the storage type used.
pub type StorageType = u128;

// Base-2 logarithm of the storage type used to represents how many bits you need to fully address every bit in that storage type
const STORAGE_SIZE: u32 = (std::mem::size_of::<StorageType>() * 8).trailing_zeros();
const STORAGE_SIZE_BITS: usize = 1 << STORAGE_SIZE;
const KEY_MASK_STORAGE_LENGTH: usize = (NUMBER_OF_KEYS + STORAGE_SIZE_BITS - 1) >> STORAGE_SIZE;

pub type KeyStates = BitVector<KEY_MASK_STORAGE_LENGTH>;

// TODO: Consider renaming to `KeyMessage` for consistency with other messages that implement `#[impl_message(..)]`
#[impl_message(Message, InputMapperMessage, KeyDown)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
	UnknownKey,

	// MouseKeys
	Lmb,
	Rmb,
	Mmb,

	// Keyboard keys
	KeyA,
	KeyB,
	KeyC,
	KeyD,
	KeyE,
	KeyF,
	KeyG,
	KeyH,
	KeyI,
	KeyJ,
	KeyK,
	KeyL,
	KeyM,
	KeyN,
	KeyO,
	KeyP,
	KeyQ,
	KeyR,
	KeyS,
	KeyT,
	KeyU,
	KeyV,
	KeyW,
	KeyX,
	KeyY,
	KeyZ,
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
	KeyEnter,
	KeyEquals,
	KeyMinus,
	KeyPlus,
	KeyShift,
	KeySpace,
	KeyControl,
	KeyCommand,
	KeyMeta,
	KeyDelete,
	KeyBackspace,
	KeyAlt,
	KeyEscape,
	KeyTab,
	KeyArrowUp,
	KeyArrowDown,
	KeyArrowLeft,
	KeyArrowRight,
	KeyLeftBracket,
	KeyRightBracket,
	KeyLeftCurlyBracket,
	KeyRightCurlyBracket,
	KeyPageUp,
	KeyPageDown,
	KeyComma,
	KeyPeriod,

	// This has to be the last element in the enum
	NumKeys,
}

impl fmt::Display for Key {
	fn fmt(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
		let key_name = format!("{:?}", self);

		let name = if &key_name[0..3] == "Key" { key_name.chars().skip(3).collect::<String>() } else { key_name };

		write!(f, "{}", name)
	}
}

pub const NUMBER_OF_KEYS: usize = Key::NumKeys as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseMotion {
	None,
	Lmb,
	Rmb,
	Mmb,
	ScrollUp,
	ScrollDown,
	Drag,
	LmbDrag,
	RmbDrag,
	MmbDrag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitVector<const LENGTH: usize>([StorageType; LENGTH]);

impl<const LENGTH: usize> BitVector<LENGTH> {
	#[inline]
	fn convert_index(bitvector_index: usize) -> (usize, StorageType) {
		let bit = 1 << (bitvector_index & (STORAGE_SIZE_BITS as StorageType - 1) as usize);
		let offset = bitvector_index >> STORAGE_SIZE;
		(offset, bit)
	}

	pub const fn new() -> Self {
		Self([0; LENGTH])
	}

	pub fn set(&mut self, bitvector_index: usize) {
		let (offset, bit) = Self::convert_index(bitvector_index);
		self.0[offset] |= bit;
	}

	pub fn unset(&mut self, bitvector_index: usize) {
		let (offset, bit) = Self::convert_index(bitvector_index);
		self.0[offset] &= !bit;
	}

	pub fn toggle(&mut self, bitvector_index: usize) {
		let (offset, bit) = Self::convert_index(bitvector_index);
		self.0[offset] ^= bit;
	}

	pub fn get(&self, bitvector_index: usize) -> bool {
		let (offset, bit) = Self::convert_index(bitvector_index);
		(self.0[offset] & bit) != 0
	}

	pub fn is_empty(&self) -> bool {
		let mut result = 0;

		for storage in self.0.iter() {
			result |= storage;
		}

		result == 0
	}

	pub fn ones(&self) -> u32 {
		let mut result = 0;

		for storage in self.0.iter() {
			result += storage.count_ones();
		}

		result
	}

	pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
		BitVectorIter::<LENGTH> { bitvector: self, iter_index: 0 }
	}
}

impl<const LENGTH: usize> Default for BitVector<LENGTH> {
	fn default() -> Self {
		Self::new()
	}
}

struct BitVectorIter<'a, const LENGTH: usize> {
	bitvector: &'a BitVector<LENGTH>,
	iter_index: usize,
}

impl<'a, const LENGTH: usize> Iterator for BitVectorIter<'a, LENGTH> {
	type Item = usize;

	fn next(&mut self) -> Option<Self::Item> {
		while self.iter_index < (STORAGE_SIZE_BITS as usize) * LENGTH {
			let bit_value = self.bitvector.get(self.iter_index);

			self.iter_index += 1;

			if bit_value {
				return Some(self.iter_index - 1);
			}
		}

		None
	}
}

impl<const LENGTH: usize> Display for BitVector<LENGTH> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		for storage in self.0.iter().rev() {
			write!(f, "{:0width$b}", storage, width = STORAGE_SIZE_BITS)?;
		}

		Ok(())
	}
}

macro_rules! bit_ops {
	($(($op:ident, $func:ident)),* $(,)?) => {
		$(
			impl<const LENGTH: usize> $op for BitVector<LENGTH> {
				type Output = Self;
				fn $func(self, right: Self) -> Self::Output {
					let mut result = Self::new();
					for ((left, right), new) in self.0.iter().zip(right.0.iter()).zip(result.0.iter_mut()) {
						*new = $op::$func(left, right);
					}
					result
				}
			}

			impl<const LENGTH: usize> $op for &BitVector<LENGTH> {
				type Output = BitVector<LENGTH>;
				fn $func(self, right: Self) -> Self::Output {
					let mut result = BitVector::<LENGTH>::new();
					for ((left, right), new) in self.0.iter().zip(right.0.iter()).zip(result.0.iter_mut()) {
						*new = $op::$func(left, right);
					}
					result
				}
			}
		)*
	};
}
macro_rules! bit_ops_assign {
	($(($op:ident, $func:ident)),* $(,)?) => {
		$(impl<const LENGTH: usize> $op for BitVector<LENGTH> {
			fn $func(&mut self, right: Self) {
				for (left, right) in self.0.iter_mut().zip(right.0.iter()) {
					$op::$func(left, right);
				}
			}
		})*
	};
}

bit_ops!((BitAnd, bitand), (BitOr, bitor), (BitXor, bitxor));
bit_ops_assign!((BitAndAssign, bitand_assign), (BitOrAssign, bitor_assign), (BitXorAssign, bitxor_assign));
