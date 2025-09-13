use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::{size_of, transmute};
use std::ptr::null_mut;
use std::sync::{Mutex, OnceLock};

use wgpu::rwh::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::{Dwm::*, Gdi::HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

pub(super) struct WindowsNativeWindowHandle {
	hwnd: HWND,
}
impl Drop for WindowsNativeWindowHandle {
	fn drop(&mut self) {
		let _ = unsafe { uninstall(self.hwnd) };
	}
}

pub(super) fn setup(window: &Window) -> WindowsNativeWindowHandle {
	let hwnd = match window.window_handle().expect("No window handle").as_raw() {
		RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut std::ffi::c_void),
		_ => panic!("Not a Win32 window"),
	};

	let dark_mode: i32 = 1;
	let _ = unsafe { DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark_mode as *const i32 as *const c_void, size_of::<i32>() as u32) };

	let system_backdrop_type: i32 = 1;
	let _ = unsafe { DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &system_backdrop_type as *const i32 as *const c_void, size_of::<i32>() as u32) };

	unsafe { ensure_helper_class() };
	let ex = WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW;
	let style = WS_POPUP;
	let helper = unsafe {
		CreateWindowExW(
			ex,
			PCWSTR(HELPER_CLASS_NAME.encode_utf16().collect::<Vec<_>>().as_ptr()),
			PCWSTR::null(),
			style,
			0,
			0,
			0,
			0,
			None,
			None,
			HINSTANCE(null_mut()),
			Some(&hwnd as *const _ as _),
		)
	}
	.expect("CreateWindowExW failed");

	let prev = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, main_window_handle_message as isize) };
	if prev == 0 {
		let _ = unsafe { DestroyWindow(helper) };
		panic!("SetWindowLongPtrW failed");
	}

	state_map().lock().unwrap().insert(
		hwnd.0 as isize,
		State {
			prev_wndproc: prev,
			helper_hwnd: helper,
		},
	);

	unsafe { position_helper(hwnd, helper) };
	let _ = unsafe { ShowWindow(helper, SW_SHOWNOACTIVATE) };

	let mut boarder_size: u32 = 1;
	let _ = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_VISIBLE_FRAME_BORDER_THICKNESS, &mut boarder_size as *mut _ as *mut _, size_of::<u32>() as u32) };
	let margins = MARGINS {
		cxLeftWidth: 0,
		cxRightWidth: 0,
		cyBottomHeight: 0,
		cyTopHeight: boarder_size as i32,
	};
	let _ = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };

	let _ = unsafe { SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER) };

	WindowsNativeWindowHandle { hwnd }
}

unsafe fn uninstall(hwnd: HWND) {
	if let Some(state) = state_map().lock().unwrap().remove(&(hwnd.0 as isize)) {
		let _ = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, state.prev_wndproc) };
		if state.helper_hwnd.0 != null_mut() {
			let _ = unsafe { DestroyWindow(state.helper_hwnd) };
		}
	}
}

const HELPER_CLASS_NAME: &str = "Helper\0";

static HELPER_CLASS_LOCK: OnceLock<u16> = OnceLock::new();
unsafe fn ensure_helper_class() {
	let _ = *HELPER_CLASS_LOCK.get_or_init(|| {
		let class_name: Vec<u16> = HELPER_CLASS_NAME.encode_utf16().collect();
		let wc = WNDCLASSW {
			style: CS_HREDRAW | CS_VREDRAW,
			lpfnWndProc: Some(helper_window_handle_message),
			hInstance: unsafe { GetModuleHandleW(None).unwrap().into() },
			hIcon: HICON::default(),
			hCursor: unsafe { LoadCursorW(HINSTANCE(null_mut()), IDC_ARROW).unwrap() },
			hbrBackground: HBRUSH::default(),
			lpszClassName: PCWSTR(class_name.as_ptr()),
			..Default::default()
		};
		unsafe { RegisterClassW(&wc) }
	});
}

fn state_map() -> &'static Mutex<HashMap<isize, State>> {
	STATE_MAP.get_or_init(|| Mutex::new(HashMap::new()))
}
static STATE_MAP: OnceLock<Mutex<HashMap<isize, State>>> = OnceLock::new();
struct State {
	prev_wndproc: isize,
	helper_hwnd: HWND,
}
unsafe impl Send for State {}
unsafe impl Sync for State {}

unsafe fn position_helper(owner: HWND, helper: HWND) {
	let mut r = RECT::default();
	let _ = unsafe { GetWindowRect(owner, &mut r) };

	const RESIZE_BAND_THICKNESS: i32 = 8;
	let x = r.left - RESIZE_BAND_THICKNESS;
	let y = r.top - RESIZE_BAND_THICKNESS;
	let w = (r.right - r.left) + RESIZE_BAND_THICKNESS * 2;
	let h = (r.bottom - r.top) + RESIZE_BAND_THICKNESS * 2;

	let _ = unsafe { SetWindowPos(helper, owner, x, y, w, h, SWP_NOACTIVATE) };
}

unsafe extern "system" fn main_window_handle_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
	match msg {
		WM_NCCALCSIZE => {
			if wparam.0 != 0 {
				return LRESULT(0);
			}
		}
		WM_MOVE | WM_MOVING | WM_SIZE | WM_SIZING | WM_WINDOWPOSCHANGED | WM_SHOWWINDOW => {
			if let Some(st) = state_map().lock().unwrap().get(&(hwnd.0 as isize)) {
				if msg == WM_SHOWWINDOW {
					if wparam.0 == 0 {
						let _ = unsafe { ShowWindow(st.helper_hwnd, SW_HIDE) };
					} else {
						let _ = unsafe { ShowWindow(st.helper_hwnd, SW_SHOWNOACTIVATE) };
					}
				}
				unsafe { position_helper(hwnd, st.helper_hwnd) };
			}
		}
		WM_DESTROY => {
			if let Some(st) = state_map().lock().unwrap().get(&(hwnd.0 as isize)) {
				if st.helper_hwnd.0 != null_mut() {
					unsafe {
						let _ = DestroyWindow(st.helper_hwnd);
					};
				}
			}
		}
		_ => {}
	}

	let prev = state_map().lock().unwrap().get(&(hwnd.0 as isize)).map(|s| s.prev_wndproc).unwrap_or(0);
	if prev != 0 {
		return unsafe { CallWindowProcW(transmute(prev), hwnd, msg, wparam, lparam) };
	}
	unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe extern "system" fn helper_window_handle_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
	match msg {
		WM_NCCREATE => {
			let cs = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
			let init = unsafe { &*(cs.lpCreateParams as *const HWND) };
			unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, init.0 as isize) };
			return LRESULT(1);
		}
		WM_ERASEBKGND => return LRESULT(1),
		WM_NCHITTEST => {
			let ht = unsafe { calculate_hit(hwnd, lparam) };
			return LRESULT(ht as isize);
		}
		WM_NCLBUTTONDOWN | WM_NCRBUTTONDOWN | WM_NCMBUTTONDOWN => {
			let owner_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut c_void;
			let owner = HWND(owner_ptr);
			if unsafe { IsWindow(owner).as_bool() } {
				let Some(wmsz) = (unsafe { calculate_resize_direction(hwnd, lparam) }) else {
					return LRESULT(0);
				};

				let _ = unsafe { SetForegroundWindow(owner) };
				let _ = unsafe { PostMessageW(owner, WM_SYSCOMMAND, WPARAM((SC_SIZE + wmsz) as usize), lparam) };
				return LRESULT(0);
			}
			return LRESULT(HTTRANSPARENT as isize);
		}
		WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
		_ => {}
	}
	unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe fn calculate_hit(helper: HWND, lparam: LPARAM) -> u32 {
	let x = (lparam.0 & 0xFFFF) as i16 as u32;
	let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as u32;

	let mut r = RECT::default();
	let _ = unsafe { GetWindowRect(helper, &mut r) };

	const RESIZE_BAND_THICKNESS: i32 = 8;
	let on_top = y < (r.top + RESIZE_BAND_THICKNESS) as u32;
	let on_right = x >= (r.right - RESIZE_BAND_THICKNESS) as u32;
	let on_bottom = y >= (r.bottom - RESIZE_BAND_THICKNESS) as u32;
	let on_left = x < (r.left + RESIZE_BAND_THICKNESS) as u32;

	match (on_top, on_right, on_bottom, on_left) {
		(true, _, _, true) => HTTOPLEFT,
		(true, true, _, _) => HTTOPRIGHT,
		(_, true, true, _) => HTBOTTOMRIGHT,
		(_, _, true, true) => HTBOTTOMLEFT,
		(true, _, _, _) => HTTOP,
		(_, true, _, _) => HTRIGHT,
		(_, _, true, _) => HTBOTTOM,
		(_, _, _, true) => HTLEFT,
		_ => HTTRANSPARENT as u32,
	}
}

unsafe fn calculate_resize_direction(helper: HWND, lparam: LPARAM) -> Option<u32> {
	match unsafe { calculate_hit(helper, lparam) } {
		HTLEFT => Some(WMSZ_LEFT),
		HTRIGHT => Some(WMSZ_RIGHT),
		HTTOP => Some(WMSZ_TOP),
		HTBOTTOM => Some(WMSZ_BOTTOM),
		HTTOPLEFT => Some(WMSZ_TOPLEFT),
		HTTOPRIGHT => Some(WMSZ_TOPRIGHT),
		HTBOTTOMLEFT => Some(WMSZ_BOTTOMLEFT),
		HTBOTTOMRIGHT => Some(WMSZ_BOTTOMRIGHT),
		_ => None,
	}
}
