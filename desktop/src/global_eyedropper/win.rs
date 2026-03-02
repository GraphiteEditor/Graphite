use graphene_std::raster::color::Color;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{Window, WindowAttributes, WindowId};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
	CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, FrameRect, GetDC, GetStockObject, ReleaseDC, SelectObject, StretchBlt, BLACK_BRUSH, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

const MAGNIFIER_RES: u32 = 11;
const MAGNIFIER_SIZE: u32 = 110;

pub(crate) struct GlobalEyedropperImpl {
	window: Option<Window>,
	primary: bool,
}

impl super::NativeEyedropper for GlobalEyedropperImpl {
	fn new() -> Self {
		Self {
			window: None,
			primary: true,
		}
	}

	fn start(&mut self, event_loop: &dyn ActiveEventLoop, primary: bool) {
		if self.is_active() {
			return;
		}

		self.primary = primary;
		let attributes = WindowAttributes::default()
			.with_title("Graphite Eyedropper")
			.with_decorations(false)
			.with_transparent(true)
			.with_always_on_top(true)
			.with_visible(false);

		match event_loop.create_window(attributes) {
			Ok(window) => {
				self.window = Some(window);
			}
			Err(e) => {
				tracing::error!("Failed to create global eyedropper window: {:?}", e);
			}
		}
	}

	fn stop(&mut self) {
		self.window = None;
	}

	fn is_active(&self) -> bool {
		self.window.is_some()
	}

	fn window_id(&self) -> Option<WindowId> {
		self.window.as_ref().map(|w| w.id())
	}

	fn update(&mut self, position: PhysicalPosition<f64>) {
		let Some(window) = &self.window else { return };

		let size = PhysicalSize::new(MAGNIFIER_SIZE, MAGNIFIER_SIZE);
		window.set_outer_position(PhysicalPosition::new(position.x - size.width as f64 / 2., position.y - size.height as f64 / 2.));
		window.set_min_surface_size(Some(size.into()));
		window.set_visible(true);
		window.request_redraw();
	}

	fn render(&self) {
		let Some(_window) = &self.window else { return };

		unsafe {
			let mut pt = Default::default();
			if GetCursorPos(&mut pt).is_err() {
				return;
			}

			let pixel_size = MAGNIFIER_SIZE / MAGNIFIER_RES;
			let half = MAGNIFIER_RES as i32 / 2;

			let desktop_dc = GetDC(HWND::default());
			let window_hwnd = self.window_hwnd();
			let window_dc = GetDC(window_hwnd);

			let mem_dc = CreateCompatibleDC(desktop_dc);
			let bitmap = CreateCompatibleBitmap(desktop_dc, MAGNIFIER_RES as i32, MAGNIFIER_RES as i32);
			let old_bitmap = SelectObject(mem_dc, bitmap);

			StretchBlt(
				mem_dc,
				0,
				0,
				MAGNIFIER_RES as i32,
				MAGNIFIER_RES as i32,
				desktop_dc,
				pt.x - half,
				pt.y - half,
				MAGNIFIER_RES as i32,
				MAGNIFIER_RES as i32,
				SRCCOPY,
			)
			.ok();

			StretchBlt(
				window_dc,
				0,
				0,
				MAGNIFIER_SIZE as i32,
				MAGNIFIER_SIZE as i32,
				mem_dc,
				0,
				0,
				MAGNIFIER_RES as i32,
				MAGNIFIER_RES as i32,
				SRCCOPY,
			)
			.ok();

			SelectObject(mem_dc, old_bitmap);
			let _ = DeleteObject(bitmap);
			let _ = DeleteDC(mem_dc);

			let mid = MAGNIFIER_RES / 2;
			let rect = RECT {
				left: (mid * pixel_size) as i32,
				top: (mid * pixel_size) as i32,
				right: ((mid + 1) * pixel_size) as i32,
				bottom: ((mid + 1) * pixel_size) as i32,
			};
			let black_brush = GetStockObject(BLACK_BRUSH);
			FrameRect(window_dc, &rect, black_brush.into());

			ReleaseDC(HWND::default(), desktop_dc);
			ReleaseDC(window_hwnd, window_dc);
		}
	}

	fn sample_color(&self) -> Option<Color> {
		unsafe {
			let mut pt = Default::default();
			if GetCursorPos(&mut pt).is_err() {
				return None;
			}

			let hdc = GetDC(HWND::default());
			let pixel = windows::Win32::Graphics::Gdi::GetPixel(hdc, pt.x, pt.y);
			ReleaseDC(HWND::default(), hdc);

			let r = (pixel.0 & 0xFF) as f32 / 255.0;
			let g = ((pixel.0 >> 8) & 0xFF) as f32 / 255.0;
			let b = ((pixel.0 >> 16) & 0xFF) as f32 / 255.0;

			Some(Color::from_rgbaf32_unchecked(r, g, b, 1.0))
		}
	}

	fn is_primary(&self) -> bool {
		self.primary
	}
}

impl GlobalEyedropperImpl {
	fn window_hwnd(&self) -> HWND {
		let Some(window) = &self.window else {
			return HWND::default();
		};
		HWND(match window.window_handle().unwrap().as_raw() {
			winit::raw_window_handle::RawWindowHandle::Win32(handle) => handle.hwnd.get() as isize,
			_ => 0,
		})
	}
}
