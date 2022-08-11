use crate::messages::portfolio::document::utility_types::misc::KeyboardPlatformLayout;
use crate::messages::prelude::*;

pub use graphene::DocumentResponse;

use bitflags::bitflags;
use serde::ser::SerializeStruct;
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

pub enum KeyPosition {
	Pressed,
	Released,
}

bitflags! {
	#[derive(Default, Serialize, Deserialize)]
	#[repr(transparent)]
	pub struct ModifierKeys: u8 {
		const SHIFT           = 0b0000_0001;
		const ALT             = 0b0000_0010;
		const CONTROL         = 0b0000_0100;
		const META_OR_COMMAND = 0b0000_1000;
	}
}

// Currently this is mostly based on the JS `KeyboardEvent.code` list: <https://www.w3.org/TR/uievents-code/>
// But in the future, especially once users can customize keyboard mappings, we should deviate more from this so we have actual symbols
// like `+` (which doesn't exist because it's the shifted version of `=` on the US keyboard, after which these scan codes are named).
// We'd ideally like to bind shortcuts to symbols, not scan codes, so the shortcut for "zoom in" is `Ctrl +` which the user can press
// (although we ignore the shift key, so the user doesn't have to press `Ctrl Shift +` on a US keyboard), even if the keyboard layout
// is for a different locale where the `+` key is somewhere entirely different, shifted or not. This would then also work for numpad `+`.
#[impl_message(Message, InputMapperMessage, KeyDown)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum Key {
	// Writing system keys
	Digit0,
	Digit1,
	Digit2,
	Digit3,
	Digit4,
	Digit5,
	Digit6,
	Digit7,
	Digit8,
	Digit9,
	//
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
	//
	Backquote,
	Backslash,
	BracketLeft,
	BracketRight,
	Comma,
	Equal,
	Minus,
	Period,
	Quote,
	Semicolon,
	Slash,

	// Functional keys
	Alt,
	Meta,
	Shift,
	Control,
	Backspace,
	CapsLock,
	ContextMenu,
	Enter,
	Space,
	Tab,

	// Control pad keys
	Delete,
	End,
	Help,
	Home,
	Insert,
	PageDown,
	PageUp,

	// Arrow pad keys
	ArrowDown,
	ArrowLeft,
	ArrowRight,
	ArrowUp,

	// Numpad keys
	// Numpad0,
	// Numpad1,
	// Numpad2,
	// Numpad3,
	// Numpad4,
	// Numpad5,
	// Numpad6,
	// Numpad7,
	// Numpad8,
	// Numpad9,
	NumLock,
	NumpadAdd,
	// NumpadBackspace,
	// NumpadClear,
	// NumpadClearEntry,
	// NumpadComma,
	// NumpadDecimal,
	// NumpadDivide,
	// NumpadEnter,
	// NumpadEqual,
	NumpadHash,
	// NumpadMemoryAdd,
	// NumpadMemoryClear,
	// NumpadMemoryRecall,
	// NumpadMemoryStore,
	// NumpadMemorySubtract,
	NumpadMultiply,
	NumpadParenLeft,
	NumpadParenRight,
	// NumpadStar,
	// NumpadSubtract,

	// Function keys
	Escape,
	F1,
	F2,
	F3,
	F4,
	F5,
	F6,
	F7,
	F8,
	F9,
	F10,
	F11,
	F12,
	F13,
	F14,
	F15,
	F16,
	F17,
	F18,
	F19,
	F20,
	F21,
	F22,
	F23,
	F24,
	Fn,
	FnLock,
	PrintScreen,
	ScrollLock,
	Pause,

	// Unidentified keys
	Unidentified,

	// Other keys that aren't part of the W3C spec
	Command,
	Lmb,
	Rmb,
	Mmb,

	// This has to be the last element in the enum
	NumKeys,
}

impl Serialize for Key {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		let key = format!("{:?}", self);
		let label = self.to_string();

		let mut state = serializer.serialize_struct("KeyWithLabel", 2)?;
		state.serialize_field("key", &key)?;
		state.serialize_field("label", &label)?;
		state.end()
	}
}

impl fmt::Display for Key {
	// TODO: Relevant key labels should be localized when we get around to implementing localization/internationalization
	fn fmt(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
		let key_name = format!("{:?}", self);

		// Writing system keys
		const DIGIT_PREFIX: &str = "Digit";
		if key_name.len() == DIGIT_PREFIX.len() + 1 && &key_name[0..DIGIT_PREFIX.len()] == "Digit" {
			return write!(f, "{}", key_name.chars().skip(DIGIT_PREFIX.len()).collect::<String>());
		}
		const KEY_PREFIX: &str = "Key";
		if key_name.len() == KEY_PREFIX.len() + 1 && &key_name[0..KEY_PREFIX.len()] == "Key" {
			return write!(f, "{}", key_name.chars().skip(KEY_PREFIX.len()).collect::<String>());
		}

		let name = match self {
			// Writing system keys
			Self::Backquote => "`",
			Self::Backslash => "\\",
			Self::BracketLeft => "[",
			Self::BracketRight => "]",
			Self::Comma => ",",
			Self::Equal => "=",
			Self::Minus => "-",
			Self::Period => ".",
			Self::Quote => "'",
			Self::Semicolon => ";",
			Self::Slash => "/",

			// Functional keys
			Self::Control => "Ctrl",

			// Control pad keys
			Self::Delete => "Del",
			Self::PageDown => "PgDn",
			Self::PageUp => "PgUp",

			// Arrow pad keys
			Self::ArrowDown => "↓",
			Self::ArrowLeft => "←",
			Self::ArrowRight => "→",
			Self::ArrowUp => "↑",

			// Numpad keys
			Self::NumpadAdd => "Numpad +",
			Self::NumpadHash => "Numpad #",
			Self::NumpadMultiply => "Numpad *",
			Self::NumpadParenLeft => "Numpad (",
			Self::NumpadParenRight => "Numpad )",

			// Function keys
			Self::Escape => "Esc",
			Self::PrintScreen => "PrtScr",

			_ => key_name.as_str(),
		};

		write!(f, "{}", name)
	}
}

pub const NUMBER_OF_KEYS: usize = Key::NumKeys as usize;

/// Only `Key`s that exist on a physical keyboard should be used.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeysGroup(pub Vec<Key>);

impl KeysGroup {
	pub fn keys_text_shortcut(&self, keyboard_platform: KeyboardPlatformLayout) -> String {
		const JOINER_MARK: &str = "+";

		let mut joined = self
			.0
			.iter()
			.map(|key| {
				let key_string = key.to_string();

				if keyboard_platform == KeyboardPlatformLayout::Mac {
					match key_string.as_str() {
						"Command" => "⌘".to_string(),
						"Control" => "⌃".to_string(),
						"Alt" => "⌥".to_string(),
						"Shift" => "⇧".to_string(),
						_ => key_string + JOINER_MARK,
					}
				} else {
					key_string + JOINER_MARK
				}
			})
			.collect::<String>();

		// Truncate to cut the joining character off the end if it's present
		if joined.ends_with(JOINER_MARK) {
			joined.truncate(joined.len() - JOINER_MARK.len());
		}

		joined
	}
}

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
