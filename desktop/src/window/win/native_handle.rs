//! Implements a Windows-specific custom window frame (no titlebar, but native boarder, shadows and resize).
//! Look and feel should be similar to a standard window.
//!
//! Implementation notes:
//! - Windows that don't use standard decorations don't get native resize handles or shadows by default.
//! - We implement resize handles (outside the main window) by creating an invisible "helper" window that
//!   is a little larger than the main window and positioned on top of it. The helper window does hit-testing
//!   and triggers native resize operations on the main window when the user clicks and drags a resize area.
//! - The helper window is a invisible window that never activates, so it doesn't steal focus from the main window.
//! - The main window needs to update the helper window's position and size whenever it moves or resizes.

use std::sync::OnceLock;
use wgpu::rwh::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;
use winit::window::Window;

#[derive(Clone)]
pub(super) struct NativeWindowHandle {
	main: HWND,
	helper: HWND,
	prev_window_message_handler: isize,
}
impl NativeWindowHandle {
	pub(super) fn new(window: &dyn Window) -> NativeWindowHandle {
		// Extract Win32 HWND from winit.
		let hwnd = match window.window_handle().expect("No window handle").as_raw() {
			RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut std::ffi::c_void),
			_ => panic!("Not a Win32 window"),
		};

		// Register the invisible helper (resize ring) window class.
		unsafe { ensure_helper_class() };

		// Create the helper as a popup tool window that never activates.
		// WS_EX_NOACTIVATE keeps focus on the main window; WS_EX_TOOLWINDOW hides it from Alt+Tab.
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
				HINSTANCE(std::ptr::null_mut()),
				// Pass the main window's HWND to WM_NCCREATE so the helper can store it.
				Some(&hwnd as *const _ as _),
			)
		}
		.expect("CreateWindowExW failed");

		// Subclass the main window.
		// https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setwindowlongptra
		let prev_window_message_handler = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, main_window_handle_message as isize) };
		if prev_window_message_handler == 0 {
			let _ = unsafe { DestroyWindow(helper) };
			panic!("SetWindowLongPtrW failed");
		}

		let inner = NativeWindowHandle {
			main: hwnd,
			helper,
			prev_window_message_handler,
		};
		registry::insert(&inner);

		// Place the helper over the main window and show it without activation.
		unsafe { position_helper(hwnd, helper) };
		let _ = unsafe { ShowWindow(helper, SW_SHOWNOACTIVATE) };

		// DwmExtendFrameIntoClientArea is needed to keep native window frame (but no titlebar).
		// https://learn.microsoft.com/windows/win32/api/dwmapi/nf-dwmapi-dwmextendframeintoclientarea
		// https://learn.microsoft.com/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute
		let mut boarder_size: u32 = 1;
		let _ = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_VISIBLE_FRAME_BORDER_THICKNESS, &mut boarder_size as *mut _ as *mut _, size_of::<u32>() as u32) };
		let margins = MARGINS {
			cxLeftWidth: 0,
			cxRightWidth: 0,
			cyBottomHeight: 0,
			cyTopHeight: boarder_size as i32,
		};
		let _ = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };

		// Force window update
		let _ = unsafe { SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER) };

		inner
	}

	pub(super) fn destroy(&self) {
		registry::remove_by_main(self.main);

		// Undo subclassing and destroy the helper window.
		let _ = unsafe { SetWindowLongPtrW(self.main, GWLP_WNDPROC, self.prev_window_message_handler) };
		if self.helper.0 != std::ptr::null_mut() {
			let _ = unsafe { DestroyWindow(self.helper) };
		}
	}
}

mod registry {
	use std::cell::RefCell;
	use windows::Win32::Foundation::HWND;

	use super::NativeWindowHandle;

	thread_local! {
		static STORE: RefCell<Vec<NativeWindowHandle>> = RefCell::new(Vec::new());
	}

	pub(super) fn find_by_main(main: HWND) -> Option<NativeWindowHandle> {
		STORE.with_borrow(|vec| vec.iter().find(|h| h.main == main).cloned())
	}
	pub(super) fn remove_by_main(main: HWND) {
		STORE.with_borrow_mut(|vec| {
			vec.retain(|h| h.main != main);
		});
	}
	pub(super) fn insert(handle: &NativeWindowHandle) {
		STORE.with_borrow_mut(|vec| {
			vec.push(handle.clone());
		});
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
			hCursor: unsafe { LoadCursorW(HINSTANCE(std::ptr::null_mut()), IDC_ARROW).unwrap() },
			// No painting; the ring is invisible.
			hbrBackground: HBRUSH::default(),
			lpszClassName: PCWSTR(class_name.as_ptr()),
			..Default::default()
		};
		unsafe { RegisterClassW(&wc) }
	});
}

// Main window message handler, called on the UI thread for every message the main window receives.
unsafe extern "system" fn main_window_handle_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
	if msg == WM_NCCALCSIZE && wparam.0 != 0 {
		// When maximized, shrink to visible frame so content doesn't extend beyond it.
		if unsafe { IsZoomed(hwnd).as_bool() } {
			let params = unsafe { &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS) };

			let dpi = unsafe { GetDpiForWindow(hwnd) };
			let size = unsafe { GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) };
			let pad = unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
			let inset = (size + pad) as i32;

			params.rgrc[0].left += inset;
			params.rgrc[0].top += inset;
			params.rgrc[0].right -= inset;
			params.rgrc[0].bottom -= inset;
		}

		// Return 0 to to tell Windows to skip the default non-client area calculation and drawing.
		return LRESULT(0);
	}

	let Some(handle) = registry::find_by_main(hwnd) else {
		return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
	};

	match msg {
		// Keep the invisible resize helper in sync with moves/resizes/visibility.
		WM_MOVE | WM_MOVING | WM_SIZE | WM_SIZING | WM_WINDOWPOSCHANGED | WM_SHOWWINDOW => {
			if msg == WM_SHOWWINDOW {
				if wparam.0 == 0 {
					let _ = unsafe { ShowWindow(handle.helper, SW_HIDE) };
				} else {
					let _ = unsafe { ShowWindow(handle.helper, SW_SHOWNOACTIVATE) };
				}
			}
			unsafe { position_helper(hwnd, handle.helper) };
		}

		// If the main window is destroyed, destroy the helper too.
		// Should only be needed if windows forcefully destroys the main window.
		WM_DESTROY => {
			let _ = unsafe { DestroyWindow(handle.helper) };
		}

		_ => {}
	}

	// Ensure the previous window message handler is not null.
	assert_ne!(handle.prev_window_message_handler, 0);

	// Call the previous window message handler, this is a standard subclassing pattern.
	let prev_window_message_handler_fn_ptr: *const () = std::ptr::without_provenance(handle.prev_window_message_handler as usize);
	let prev_window_message_handler_fn = unsafe { std::mem::transmute::<_, _>(prev_window_message_handler_fn_ptr) };
	return unsafe { CallWindowProcW(Some(prev_window_message_handler_fn), hwnd, msg, wparam, lparam) };
}

// Helper window message handler, called on the UI thread for every message the helper window receives.
unsafe extern "system" fn helper_window_handle_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
	match msg {
		// Helper window creation, should be the first message that the helper window receives.
		WM_NCCREATE => {
			// Main window HWND is provided when creating the helper window with `CreateWindowExW`
			// Save main window HWND in GWLP_USERDATA so we can extract it later
			let crate_struct = lparam.0 as *const CREATESTRUCTW;
			let create_param = unsafe { (*crate_struct).lpCreateParams as *const HWND };
			unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create_param).0 as isize) };
			return LRESULT(1);
		}

		// Invisible; no background erase.
		WM_ERASEBKGND => return LRESULT(1),

		// Tell windows what resize areas we are hitting, this is used to decide what cursor to show.
		WM_NCHITTEST => {
			let ht = unsafe { calculate_hit(hwnd, lparam) };
			return LRESULT(ht as isize);
		}

		// This starts the system's resize loop for the main window if a resize area is hit.
		// Helper window button down translates to SC_SIZE | WMSZ_* on the main window.
		WM_NCLBUTTONDOWN | WM_NCRBUTTONDOWN | WM_NCMBUTTONDOWN => {
			// Extract the main window's HWND from GWLP_USERDATA that we saved earlier.
			let main_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut std::ffi::c_void;
			let main = HWND(main_ptr);
			if unsafe { IsWindow(main).as_bool() } {
				let Some(wmsz) = (unsafe { calculate_resize_direction(hwnd, lparam) }) else {
					return LRESULT(0);
				};

				// Ensure that the main window can receive WM_SYSCOMMAND.
				let _ = unsafe { SetForegroundWindow(main) };

				// Start sizing on the main window in the calculated direction. (SC_SIZE + WMSZ_*)
				let _ = unsafe { PostMessageW(main, WM_SYSCOMMAND, WPARAM((SC_SIZE + wmsz) as usize), lparam) };
			}
			return LRESULT(0);
		}

		// Never activate the helper window, allows all inputs that don't hit the resize areas to pass through.
		WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
		_ => {}
	}
	unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

// Position the helper window to match the main window's location and size (plus the resize band size).
unsafe fn position_helper(main: HWND, helper: HWND) {
	let mut r = RECT::default();
	let _ = unsafe { GetWindowRect(main, &mut r) };

	const RESIZE_BAND_SIZE: i32 = 8;
	let x = r.left - RESIZE_BAND_SIZE;
	let y = r.top - RESIZE_BAND_SIZE;
	let w = (r.right - r.left) + RESIZE_BAND_SIZE * 2;
	let h = (r.bottom - r.top) + RESIZE_BAND_SIZE * 2;

	let _ = unsafe { SetWindowPos(helper, main, x, y, w, h, SWP_NOACTIVATE) };
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
