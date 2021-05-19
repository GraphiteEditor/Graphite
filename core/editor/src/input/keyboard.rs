use bitflags::bitflags;

#[derive(Debug, Default)]
pub struct KeyState {
	depressed: bool,
	// time of last press
	// mod keys held down while pressing
	// …
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Key {
	UnknownKey,
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
}

bitflags! {
	#[derive(Default)]
	#[repr(transparent)]
	pub struct ModKeys: u8 {
		const CONTROL = 0b0000_0001;
		const SHIFT   = 0b0000_0010;
		const ALT     = 0b0000_0100;
	}
}
