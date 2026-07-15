use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_keyboard_handler_t, cef_base_ref_counted_t, cef_event_flags_t, cef_key_event_type_t};
use cef::{Browser, ImplBrowser, ImplBrowserHost, ImplFrame, ImplKeyboardHandler, KeyEvent, WrapKeyboardHandler};

const SHIFT: u32 = cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0;
const CONTROL: u32 = cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0;
const ALT: u32 = cef_event_flags_t::EVENTFLAG_ALT_DOWN.0;
const COMMAND: u32 = cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0;
const MODIFIER_MASK: u32 = SHIFT | CONTROL | ALT | COMMAND;

const VK_END: i32 = 0x23;
const VK_HOME: i32 = 0x24;
const VK_UP: i32 = 0x26;
const VK_DOWN: i32 = 0x28;
const VK_A: i32 = 0x41;
const VK_C: i32 = 0x43;
const VK_V: i32 = 0x56;
const VK_X: i32 = 0x58;
const VK_Z: i32 = 0x5A;

const KVK_HOME: i32 = 0x73;
const KVK_END: i32 = 0x77;
const NS_HOME_FUNCTION_KEY: u16 = 0xF729;
const NS_END_FUNCTION_KEY: u16 = 0xF72B;

pub(crate) struct KeyboardHandlerImpl {
	object: *mut RcImpl<_cef_keyboard_handler_t, Self>,
}
impl KeyboardHandlerImpl {
	pub(crate) fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}
}

impl ImplKeyboardHandler for KeyboardHandlerImpl {
	fn on_key_event(&self, browser: Option<&mut Browser>, event: Option<&KeyEvent>, _os_event: *mut u8) -> std::ffi::c_int {
		let (Some(browser), Some(event)) = (browser, event) else { return 0 };
		if event.type_ != cef_key_event_type_t::KEYEVENT_RAWKEYDOWN.into() {
			return 0;
		}

		let modifiers = event.modifiers & MODIFIER_MASK;
		let shift = modifiers & SHIFT != 0;

		if modifiers & COMMAND != 0 && modifiers & (CONTROL | ALT) == 0 {
			let Some(frame) = browser.focused_frame() else { return 0 };
			match (event.windows_key_code, shift) {
				(VK_A, false) => frame.select_all(),
				(VK_C, false) => frame.copy(),
				(VK_V, false) => frame.paste(),
				(VK_V, true) => frame.paste_and_match_style(),
				(VK_X, false) => frame.cut(),
				(VK_Z, false) => frame.undo(),
				(VK_Z, true) => frame.redo(),
				_ => return 0,
			}
			return 1;
		}

		if modifiers == SHIFT | ALT {
			let (windows_key_code, native_key_code, character) = match event.windows_key_code {
				VK_UP => (VK_HOME, KVK_HOME, NS_HOME_FUNCTION_KEY),
				VK_DOWN => (VK_END, KVK_END, NS_END_FUNCTION_KEY),
				_ => return 0,
			};
			let Some(host) = browser.host() else { return 0 };
			host.send_key_event(Some(&KeyEvent {
				type_: cef_key_event_type_t::KEYEVENT_RAWKEYDOWN.into(),
				modifiers: SHIFT,
				windows_key_code,
				native_key_code,
				character,
				unmodified_character: character,
				..Default::default()
			}));
			return 1;
		}

		0
	}

	fn get_raw(&self) -> *mut _cef_keyboard_handler_t {
		self.object.cast()
	}
}

impl Clone for KeyboardHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for KeyboardHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapKeyboardHandler for KeyboardHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_keyboard_handler_t, Self>) {
		self.object = object;
	}
}
