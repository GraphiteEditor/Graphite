use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::raw_window_handle::HasWindowHandle;
use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC, GetPixel, COLORREF, BitBlt, CreateCompatibleDC, CreateCompatibleBitmap, SelectObject, DeleteDC, DeleteObject, SRCCOPY, HDC};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetDesktopWindow, GetWindowRect};
use graphene_std::raster::color::Color;

pub struct GlobalEyedropper {
	window: Option<Window>,
	primary: bool,
}

impl GlobalEyedropper {
	pub fn new() -> Self {
		Self {
			window: None,
			primary: true,
		}
	}

	pub fn start(&mut self, event_loop: &dyn ActiveEventLoop, primary: bool) {
		self.primary = primary;
		let attributes = WindowAttributes::default()
			.with_title("Graphite Eyedropper")
			.with_decorations(false)
			.with_transparent(true)
			.with_always_on_top(true)
			.with_visible(false); // We'll show it and move it in the first update

		match event_loop.create_window(attributes) {
			Ok(window) => {
				self.window = Some(window);
			}
			Err(e) => {
				tracing::error!("Failed to create global eyedropper window: {:?}", e);
			}
		}
	}

	pub fn stop(&mut self) {
		self.window = None;
	}

	pub fn is_active(&self) -> bool {
		self.window.is_some()
	}

	pub fn window_id(&self) -> Option<WindowId> {
		self.window.as_ref().map(|w| w.id())
	}

	pub fn update(&mut self, position: PhysicalPosition<f64>) {
		let Some(window) = &self.window else { return };

		let size = PhysicalSize::new(110, 110);
		window.set_outer_position(PhysicalPosition::new(position.x - size.width as f64 / 2., position.y - size.height as f64 / 2.));
		window.set_min_surface_size(Some(size.into()));
		window.set_visible(true);
		window.request_redraw();
	}

	pub fn render(&self) {
		let Some(window) = &self.window else { return };
		let size = window.inner_size();

		unsafe {
			let mut pt = Default::default();
			if GetCursorPos(&mut pt).is_err() {
				return;
			}

			let res = 11;
			let pixel_size = size.width / res;

			let desktop_dc = GetDC(HWND::default());
			let window_dc = GetDC(HWND(match window.window_handle().unwrap().as_raw() {
				winit::raw_window_handle::RawWindowHandle::Win32(handle) => handle.hwnd.get() as isize,
				_ => 0,
			}));

			for y in 0..res {
				for x in 0..res {
					let sx = pt.x - (res as i32 / 2) + x as i32;
					let sy = pt.y - (res as i32 / 2) + y as i32;
					let color = GetPixel(desktop_dc, sx, sy);

					let rect = RECT {
						left: (x * pixel_size) as i32,
						top: (y * pixel_size) as i32,
						right: ((x + 1) * pixel_size) as i32,
						bottom: ((y + 1) * pixel_size) as i32,
					};
					windows::Win32::Graphics::Gdi::FillRect(window_dc, &rect, windows::Win32::Graphics::Gdi::HBRUSH((color.0 + 1) as isize));
				}
			}

			let mid = res / 2;
			let rect = RECT {
				left: (mid * pixel_size) as i32,
				top: (mid * pixel_size) as i32,
				right: ((mid + 1) * pixel_size) as i32,
				bottom: ((mid + 1) * pixel_size) as i32,
			};
			windows::Win32::Graphics::Gdi::FrameRect(window_dc, &rect, windows::Win32::Graphics::Gdi::HBRUSH(1));

			ReleaseDC(HWND::default(), desktop_dc);
			ReleaseDC(HWND(match window.window_handle().unwrap().as_raw() {
				winit::raw_window_handle::RawWindowHandle::Win32(handle) => handle.hwnd.get() as isize,
				_ => 0,
			}), window_dc);
		}
	}

	pub fn sample_color(&self) -> Option<Color> {
		unsafe {
			let mut pt = Default::default();
			if GetCursorPos(&mut pt).is_err() {
				return None;
			}

			let hdc = GetDC(HWND::default());
			let pixel = GetPixel(hdc, pt.x, pt.y);
			ReleaseDC(HWND::default(), hdc);

			let r = (pixel.0 & 0xFF) as f32 / 255.0;
			let g = ((pixel.0 >> 8) & 0xFF) as f32 / 255.0;
			let b = ((pixel.0 >> 16) & 0xFF) as f32 / 255.0;

			Some(Color::from_rgbaf32(r, g, b, 1.0).unwrap())
		}
	}

	pub fn is_primary(&self) -> bool {
		self.primary
	}
}
