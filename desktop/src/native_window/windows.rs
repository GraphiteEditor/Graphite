//! # Hybrid borderless window with an invisible resize band
//!
//! This module turns a standard Win32 window into a custom-framed window while
//! preserving native resize behavior and shadows by surrounding it with an
//! **invisible helper window** (an 8px “ring”). The ring performs hit-testing
//! outside the visible bounds and then triggers the system’s resize/move loop
//! **on the main window**, so you get OS-accurate resizing, snapping, and
//! cursors without drawing a standard caption or border.
//!
//! Key ideas:
//! - We extend/glass the client frame with DWM to avoid the system-drawn title bar,
//!   but keep modern visuals (e.g., dark caption, Mica).  
//!   Docs: DWM custom frame & extending client area.  
//!   <https://learn.microsoft.com/windows/win32/dwm/customframe>  
//!   <https://learn.microsoft.com/windows/win32/api/dwmapi/nf-dwmapi-dwmextendframeintoclientarea>
//! - We subclass the main window proc to manage layout and keep the helper synced.  
//!   <https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setwindowlongptra>
//! - The helper window uses `WM_NCHITTEST` to classify edges/corners (`HT*`) and,
//!   on mouse down, starts the system resize loop on the owner via `WM_SYSCOMMAND`
//!   with `SC_SIZE | WMSZ_*`.  
//!   <https://learn.microsoft.com/windows/win32/inputdev/wm-nchittest>  
//!   <https://learn.microsoft.com/windows/win32/menurc/wm-syscommand>
//! - The helper is created with `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` so it never
//!   activates or shows in Alt+Tab, but still receives mouse input.  
//!   <https://learn.microsoft.com/windows/win32/winmsg/extended-window-styles>
//! - `DWMWA_VISIBLE_FRAME_BORDER_THICKNESS` helps match system metrics when
//!   extending frame or aligning visuals.  
//!   <https://learn.microsoft.com/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute>
//!
//! This pattern avoids trying to “extend hit-testing” beyond an HWND’s bounds,
//! which Win32 does not support directly; instead we *place another HWND there*
//! and forward the action to the owner.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::{size_of, transmute};
use std::ptr::null_mut;
use std::sync::{Mutex, OnceLock, RwLock};
use std::thread::ThreadId;

use wgpu::rwh::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::{Dwm::*, Gdi::HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

pub(super) struct WindowsNativeWindowHandle {
	inner: WindowsNativeWindowHandleInner,
}

#[derive(Clone)]
struct WindowsNativeWindowHandleInner {
	owner: HWND,
	helper: HWND,
	prev_window_message_handler: isize,
}

pub(super) fn setup(window: &Window) -> WindowsNativeWindowHandle {
	// Extract Win32 HWND from winit.
	let hwnd = match window.window_handle().expect("No window handle").as_raw() {
		RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut std::ffi::c_void),
		_ => panic!("Not a Win32 window"),
	};

	// Ask DWM to draw a dark caption (when applicable).
	// DWMWA_USE_IMMERSIVE_DARK_MODE is supported on recent Windows 10+ builds.
	// Ref: https://learn.microsoft.com/windows/apps/desktop/modernize/ui/apply-windows-themes
	let dark_mode: i32 = 1;
	let _ = unsafe { DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark_mode as *const i32 as *const c_void, size_of::<i32>() as u32) };

	// Enable a system backdrop material (e.g., Mica) behind the non-client region.
	// Ref: DWMWA_SYSTEMBACKDROP_TYPE
	// https://learn.microsoft.com/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type
	let system_backdrop_type: i32 = 1;
	let _ = unsafe { DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &system_backdrop_type as *const i32 as *const c_void, size_of::<i32>() as u32) };

	// Register the invisible helper (resize ring) window class.
	unsafe { ensure_helper_class() };

	// Create the helper as a popup tool window that never activates.
	// WS_EX_NOACTIVATE keeps focus on the owner; WS_EX_TOOLWINDOW hides it from Alt+Tab.
	// https://learn.microsoft.com/windows/win32/winmsg/extended-window-styles
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
			// Pass the owner HWND to WM_NCCREATE so the helper can store it.
			Some(&hwnd as *const _ as _),
		)
	}
	.expect("CreateWindowExW failed");

	// Subclass the main window so we can react to move/size/show and keep the
	// helper ring positioned.
	// https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setwindowlongptra
	let prev = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, main_window_handle_message as isize) };
	if prev == 0 {
		let _ = unsafe { DestroyWindow(helper) };
		panic!("SetWindowLongPtrW failed");
	}

	// Place the helper ring around the owner and show it without activation.
	unsafe { position_helper(hwnd, helper) };
	let _ = unsafe { ShowWindow(helper, SW_SHOWNOACTIVATE) };

	// Query the system-visible frame border thickness (varies by DPI) and
	// extend the frame into the client area to blend system and custom visuals.
	// https://learn.microsoft.com/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute
	// https://learn.microsoft.com/windows/win32/api/dwmapi/nf-dwmapi-dwmextendframeintoclientarea
	let mut boarder_size: u32 = 1;
	let _ = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_VISIBLE_FRAME_BORDER_THICKNESS, &mut boarder_size as *mut _ as *mut _, size_of::<u32>() as u32) };
	let margins = MARGINS {
		cxLeftWidth: 0,
		cxRightWidth: 0,
		cyBottomHeight: 0,
		cyTopHeight: boarder_size as i32,
	};
	let _ = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };

	// Force the non-client metrics to be recalculated after style/DWM changes.
	let _ = unsafe { SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER) };

	let inner = WindowsNativeWindowHandleInner {
		owner: hwnd,
		helper: helper,
		prev_window_message_handler: prev,
	};

	registry::insert(&inner);

	WindowsNativeWindowHandle { inner }
}

impl Drop for WindowsNativeWindowHandle {
	fn drop(&mut self) {
		// Undo subclassing and destroy the helper ring.
		registry::remove_by_owner(self.inner.owner);

		let _ = unsafe { SetWindowLongPtrW(self.inner.owner, GWLP_WNDPROC, self.inner.prev_window_message_handler) };
		if self.inner.helper.0 != null_mut() {
			let _ = unsafe { DestroyWindow(self.inner.helper) };
		}
	}
}

const HELPER_CLASS_NAME: &str = "Helper\0";

static HELPER_CLASS_LOCK: OnceLock<u16> = OnceLock::new();
unsafe fn ensure_helper_class() {
	// Register a window class for the invisible resize helper.
	let _ = *HELPER_CLASS_LOCK.get_or_init(|| {
		let class_name: Vec<u16> = HELPER_CLASS_NAME.encode_utf16().collect();
		let wc = WNDCLASSW {
			style: CS_HREDRAW | CS_VREDRAW,
			lpfnWndProc: Some(helper_window_handle_message),
			hInstance: unsafe { GetModuleHandleW(None).unwrap().into() },
			hIcon: HICON::default(),
			hCursor: unsafe { LoadCursorW(HINSTANCE(null_mut()), IDC_ARROW).unwrap() },
			// No painting; the ring is invisible.
			hbrBackground: HBRUSH::default(),
			lpszClassName: PCWSTR(class_name.as_ptr()),
			..Default::default()
		};
		unsafe { RegisterClassW(&wc) }
	});
}

mod registry {
	use std::cell::RefCell;
	use windows::Win32::Foundation::HWND;

	use crate::native_window::windows::WindowsNativeWindowHandleInner;

	thread_local! {
		static STORE: RefCell<Vec<WindowsNativeWindowHandleInner>> = RefCell::new(Vec::new());
	}

	pub(super) fn find_by_helper(helper: HWND) -> Option<WindowsNativeWindowHandleInner> {
		STORE.with_borrow(|vec| vec.iter().find(|h| h.helper == helper).cloned())
	}
	pub(super) fn find_by_owner(owner: HWND) -> Option<WindowsNativeWindowHandleInner> {
		STORE.with_borrow(|vec| vec.iter().find(|h| h.owner == owner).cloned())
	}
	pub(super) fn remove_by_owner(owner: HWND) {
		STORE.with_borrow_mut(|vec| {
			vec.retain(|h| h.owner != owner);
		});
	}
	pub(super) fn insert(handle: &WindowsNativeWindowHandleInner) {
		STORE.with_borrow_mut(|vec| {
			vec.push(handle.clone());
		});
	}
}

// Position the helper window to match the owner’s location and size plus the resize band size.
unsafe fn position_helper(owner: HWND, helper: HWND) {
	let mut r = RECT::default();
	let _ = unsafe { GetWindowRect(owner, &mut r) };

	const RESIZE_BAND_SIZE: i32 = 8;
	let x = r.left - RESIZE_BAND_SIZE;
	let y = r.top - RESIZE_BAND_SIZE;
	let w = (r.right - r.left) + RESIZE_BAND_SIZE * 2;
	let h = (r.bottom - r.top) + RESIZE_BAND_SIZE * 2;

	let _ = unsafe { SetWindowPos(helper, owner, x, y, w, h, SWP_NOACTIVATE) };
}

unsafe extern "system" fn main_window_handle_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
	match msg {
		// Return 0 to to tell Windows to skip the default non-client area calculation and drawing.
		WM_NCCALCSIZE => {
			if wparam.0 != 0 {
				return LRESULT(0);
			}
		}

		// Keep the invisible resize helper in sync with moves/resizes/visibility.
		WM_MOVE | WM_MOVING | WM_SIZE | WM_SIZING | WM_WINDOWPOSCHANGED | WM_SHOWWINDOW => {
			if let Some(handle) = registry::find_by_owner(hwnd) {
				if msg == WM_SHOWWINDOW {
					if wparam.0 == 0 {
						let _ = unsafe { ShowWindow(handle.helper, SW_HIDE) };
					} else {
						let _ = unsafe { ShowWindow(handle.helper, SW_SHOWNOACTIVATE) };
					}
				}
				unsafe { position_helper(hwnd, handle.helper) };
			}
		}

		// If the owner is destroyed, destroy the helper too.
		WM_DESTROY => {
			if let Some(handle) = registry::find_by_owner(hwnd) {
				if handle.helper.0 != null_mut() {
					unsafe {
						let _ = DestroyWindow(handle.helper);
					};
				}
			}
		}
		_ => {}
	}

	// Call the previous window procedure, this is standard subclassing pattern.
	let prev = registry::find_by_owner(hwnd).map(|h| h.prev_window_message_handler).unwrap_or(0);
	if prev != 0 {
		return unsafe { CallWindowProcW(transmute(prev), hwnd, msg, wparam, lparam) };
	}

	// Fall back to the default window procedure, happens when subclass initialization failed.
	unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe extern "system" fn helper_window_handle_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
	match msg {
		// Helper window creation, should be the first message that the helper window receives.
		WM_NCCREATE => {
			// Save owner HWND in GWLP_USERDATA so we can extract it later
			let cs = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
			let init = unsafe { &*(cs.lpCreateParams as *const HWND) };
			unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, init.0 as isize) };
			return LRESULT(1);
		}

		// Invisible; no background erase.
		WM_ERASEBKGND => return LRESULT(1),

		// Tell windows what resize areas we are hitting, this is used to decide what cursor to show.
		WM_NCHITTEST => {
			let ht = unsafe { calculate_hit(hwnd, lparam) };
			return LRESULT(ht as isize);
		}

		// This starts the system's modal resize loop for the owner window if a resize area is hit.
		// Helper window button down, translates to SC_SIZE | WMSZ_* on the owner.
		WM_NCLBUTTONDOWN | WM_NCRBUTTONDOWN | WM_NCMBUTTONDOWN => {
			// Extract the owner HWND from GWLP_USERDATA that we saved earlier.
			let owner_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut c_void;
			let owner = HWND(owner_ptr);
			if unsafe { IsWindow(owner).as_bool() } {
				let Some(wmsz) = (unsafe { calculate_resize_direction(hwnd, lparam) }) else {
					return LRESULT(0);
				};

				// Ensure that the owner can receive WM_SYSCOMMAND.
				let _ = unsafe { SetForegroundWindow(owner) };

				// Start sizing on the owner in the calculated direction. (SC_SIZE + WMSZ_*)
				let _ = unsafe { PostMessageW(owner, WM_SYSCOMMAND, WPARAM((SC_SIZE + wmsz) as usize), lparam) };
			}
			return LRESULT(0);
		}

		// Never activate the helper window, allows all inputs that don't hit the resize areas to pass through.
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
