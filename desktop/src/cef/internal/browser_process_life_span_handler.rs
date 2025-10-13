use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_life_span_handler_t, cef_base_ref_counted_t};
use cef::{ImplLifeSpanHandler, WrapLifeSpanHandler};

pub(crate) struct BrowserProcessLifeSpanHandlerImpl {
	object: *mut RcImpl<_cef_life_span_handler_t, Self>,
}
impl BrowserProcessLifeSpanHandlerImpl {
	pub(crate) fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}
}

impl ImplLifeSpanHandler for BrowserProcessLifeSpanHandlerImpl {
	fn on_before_popup(
		&self,
		_browser: Option<&mut cef::Browser>,
		_frame: Option<&mut cef::Frame>,
		_popup_id: ::std::os::raw::c_int,
		target_url: Option<&cef::CefString>,
		_target_frame_name: Option<&cef::CefString>,
		_target_disposition: cef::WindowOpenDisposition,
		_user_gesture: ::std::os::raw::c_int,
		_popup_features: Option<&cef::PopupFeatures>,
		_window_info: Option<&mut cef::WindowInfo>,
		_client: Option<&mut Option<cef::Client>>,
		_settings: Option<&mut cef::BrowserSettings>,
		_extra_info: Option<&mut Option<cef::DictionaryValue>>,
		_no_javascript_access: Option<&mut ::std::os::raw::c_int>,
	) -> ::std::os::raw::c_int {
		let target = target_url.map(|url| url.to_string()).unwrap_or("unknown".to_string());
		tracing::error!("Browser tried to open a popup at URL: {}", target);

		// Deny any popup by returning 1
		1
	}

	fn get_raw(&self) -> *mut _cef_life_span_handler_t {
		self.object.cast()
	}
}

impl Clone for BrowserProcessLifeSpanHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for BrowserProcessLifeSpanHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapLifeSpanHandler for BrowserProcessLifeSpanHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_life_span_handler_t, Self>) {
		self.object = object;
	}
}
