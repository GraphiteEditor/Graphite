pub trait ToNativeKeycode {
	fn to_native_keycode(&self) -> i32;
}

impl ToNativeKeycode for winit::keyboard::PhysicalKey {
	fn to_native_keycode(&self) -> i32 {
		use winit::platform::scancode::PhysicalKeyExtScancode;

		#[cfg(target_os = "linux")]
		{
			self.to_scancode().map(|evdev| (evdev + 8) as i32).unwrap_or(0)
		}
		#[cfg(any(target_os = "macos", target_os = "windows"))]
		{
			self.to_scancode().map(|c| c as i32).unwrap_or(0)
		}
	}
}

// Windows Virtual keyboard binary representation
pub(crate) trait ToVKBits {
	fn to_vk_bits(&self) -> i32;
}

macro_rules! map_enum {
	($target:expr, $enum:ident, $( ($code:expr, $variant:ident), )+ ) => {
		match $target {
			$(
				$enum::$variant => $code,
			)+
			_ => 0,
		}
	};
}

impl ToVKBits for winit::keyboard::NamedKey {
	fn to_vk_bits(&self) -> i32 {
		use winit::keyboard::NamedKey;
		map_enum!(
			self,
			NamedKey,
			(0x12, Alt),
			(0xA5, AltGraph),
			(0x14, CapsLock),
			(0x11, Control),
			(0x90, NumLock),
			(0x91, ScrollLock),
			(0x10, Shift),
			(0x5B, Meta),
			(0x0D, Enter),
			(0x09, Tab),
			(0x28, ArrowDown),
			(0x25, ArrowLeft),
			(0x27, ArrowRight),
			(0x26, ArrowUp),
			(0x23, End),
			(0x24, Home),
			(0x22, PageDown),
			(0x21, PageUp),
			(0x08, Backspace),
			(0x0C, Clear),
			(0xF7, CrSel),
			(0x2E, Delete),
			(0xF9, EraseEof),
			(0xF8, ExSel),
			(0x2D, Insert),
			(0x1E, Accept),
			(0xF6, Attn),
			(0x03, Cancel),
			(0x5D, ContextMenu),
			(0x1B, Escape),
			(0x2B, Execute),
			(0x2F, Help),
			(0x13, Pause),
			(0xFA, Play),
			(0x5D, Props),
			(0x29, Select),
			(0xFB, ZoomIn),
			(0xFB, ZoomOut),
			(0x2C, PrintScreen),
			(0x5F, Standby),
			(0x1C, Convert),
			(0x18, FinalMode),
			(0x1F, ModeChange),
			(0x1D, NonConvert),
			(0xE5, Process),
			(0x15, HangulMode),
			(0x19, HanjaMode),
			(0x17, JunjaMode),
			(0x15, KanaMode),
			(0x19, KanjiMode),
			(0xB0, MediaFastForward),
			(0xB3, MediaPause),
			(0xB3, MediaPlay),
			(0xB3, MediaPlayPause),
			(0xB1, MediaRewind),
			(0xB2, MediaStop),
			(0xB0, MediaTrackNext),
			(0xB1, MediaTrackPrevious),
			(0x2A, Print),
			(0xAE, AudioVolumeDown),
			(0xAF, AudioVolumeUp),
			(0xAD, AudioVolumeMute),
			(0xB6, LaunchApplication1),
			(0xB7, LaunchApplication2),
			(0xB4, LaunchMail),
			(0xB5, LaunchMediaPlayer),
			(0xB5, LaunchMusicPlayer),
			(0xA6, BrowserBack),
			(0xAB, BrowserFavorites),
			(0xA7, BrowserForward),
			(0xAC, BrowserHome),
			(0xA8, BrowserRefresh),
			(0xAA, BrowserSearch),
			(0xA9, BrowserStop),
			(0xFB, ZoomToggle),
			(0x70, F1),
			(0x71, F2),
			(0x72, F3),
			(0x73, F4),
			(0x74, F5),
			(0x75, F6),
			(0x76, F7),
			(0x77, F8),
			(0x78, F9),
			(0x79, F10),
			(0x7A, F11),
			(0x7B, F12),
			(0x7C, F13),
			(0x7D, F14),
			(0x7E, F15),
			(0x7F, F16),
			(0x80, F17),
			(0x81, F18),
			(0x82, F19),
			(0x83, F20),
			(0x84, F21),
			(0x85, F22),
			(0x86, F23),
			(0x87, F24),
		)
	}
}

macro_rules! map {
	($target:expr, $( ($code:expr, $variant:literal), )+ ) => {
		match $target {
			$(
				$variant => $code,
			)+
			_ => 0,
		}
	};
}

impl ToVKBits for char {
	fn to_vk_bits(&self) -> i32 {
		map!(
			self,
			(0x41, 'a'),
			(0x42, 'b'),
			(0x43, 'c'),
			(0x44, 'd'),
			(0x45, 'e'),
			(0x46, 'f'),
			(0x47, 'g'),
			(0x48, 'h'),
			(0x49, 'i'),
			(0x4a, 'j'),
			(0x4b, 'k'),
			(0x4c, 'l'),
			(0x4d, 'm'),
			(0x4e, 'n'),
			(0x4f, 'o'),
			(0x50, 'p'),
			(0x51, 'q'),
			(0x52, 'r'),
			(0x53, 's'),
			(0x54, 't'),
			(0x55, 'u'),
			(0x56, 'v'),
			(0x57, 'w'),
			(0x58, 'x'),
			(0x59, 'y'),
			(0x5a, 'z'),
			(0x41, 'A'),
			(0x42, 'B'),
			(0x43, 'C'),
			(0x44, 'D'),
			(0x45, 'E'),
			(0x46, 'F'),
			(0x47, 'G'),
			(0x48, 'H'),
			(0x49, 'I'),
			(0x4a, 'J'),
			(0x4b, 'K'),
			(0x4c, 'L'),
			(0x4d, 'M'),
			(0x4e, 'N'),
			(0x4f, 'O'),
			(0x50, 'P'),
			(0x51, 'Q'),
			(0x52, 'R'),
			(0x53, 'S'),
			(0x54, 'T'),
			(0x55, 'U'),
			(0x56, 'V'),
			(0x57, 'W'),
			(0x58, 'X'),
			(0x59, 'Y'),
			(0x5a, 'Z'),
			(0x31, '1'),
			(0x32, '2'),
			(0x33, '3'),
			(0x34, '4'),
			(0x35, '5'),
			(0x36, '6'),
			(0x37, '7'),
			(0x38, '8'),
			(0x39, '9'),
			(0x30, '0'),
			(0x31, '!'),
			(0x32, '@'),
			(0x33, '#'),
			(0x34, '$'),
			(0x35, '%'),
			(0x36, '^'),
			(0x37, '&'),
			(0x38, '*'),
			(0x39, '('),
			(0x30, ')'),
			(0xC0, '`'),
			(0xC0, '~'),
			(0xBD, '-'),
			(0xBD, '_'),
			(0xBB, '='),
			(0xBB, '+'),
			(0xDB, '['),
			(0xDB, '{'),
			(0xDD, ']'),
			(0xDD, '}'),
			(0xDC, '\\'),
			(0xDC, '|'),
			(0xBA, ';'),
			(0xBA, ':'),
			(0xBC, ','),
			(0xBC, '<'),
			(0xBE, '.'),
			(0xBE, '>'),
			(0xDE, '\''),
			(0xDE, '"'),
			(0xBF, '/'),
			(0xBF, '?'),
			(0x20, ' '),
		)
	}
}
