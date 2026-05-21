use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_app_t, cef_base_ref_counted_t};
use cef::{BrowserProcessHandler, CefString, ImplApp, ImplCommandLine, SchemeRegistrar, WrapApp};

use super::browser_process_handler::BrowserProcessHandlerImpl;
use super::scheme_handler_factory::SchemeHandlerFactoryImpl;
use crate::cef::CefEventHandler;

pub(crate) struct BrowserProcessAppImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_app_t, Self>,
	event_handler: H,
	accelerated_paint: bool,
}
impl<H: CefEventHandler> BrowserProcessAppImpl<H> {
	pub(crate) fn new(event_handler: H, accelerated_paint: bool) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_handler,
			accelerated_paint,
		}
	}
}

impl<H: CefEventHandler> ImplApp for BrowserProcessAppImpl<H> {
	fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
		Some(BrowserProcessHandler::new(BrowserProcessHandlerImpl::new(self.event_handler.duplicate())))
	}

	fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
		SchemeHandlerFactoryImpl::<H>::register_schemes(registrar);
	}

	fn on_before_command_line_processing(&self, _process_type: Option<&cef::CefString>, command_line: Option<&mut cef::CommandLine>) {
		if let Some(cmd) = command_line {
			cmd.append_switch_with_value(Some(&"renderer-process-limit".into()), Some(&"1".into()));
			cmd.append_switch_with_value(Some(&"password-store".into()), Some(&"basic".into()));
			cmd.append_switch_with_value(Some(&"disk-cache-size".into()), Some(&"0".into()));
			cmd.append_switch(Some(&"no-sandbox".into()));
			cmd.append_switch(Some(&"no-first-run".into()));
			cmd.append_switch(Some(&"noerrdialogs".into()));
			cmd.append_switch(Some(&"no-default-browser-check".into()));
			cmd.append_switch(Some(&"mute-audio".into()));
			cmd.append_switch(Some(&"use-fake-device-for-media-stream".into()));
			cmd.append_switch(Some(&"incognito".into()));
			cmd.append_switch(Some(&"disable-sync".into()));
			cmd.append_switch(Some(&"disable-file-system".into()));
			cmd.append_switch(Some(&"disable-component-update".into()));
			cmd.append_switch(Some(&"disable-geolocation".into()));
			cmd.append_switch(Some(&"disable-notifications".into()));
			cmd.append_switch(Some(&"disable-background-networking".into()));
			cmd.append_switch(Some(&"disable-default-apps".into()));
			cmd.append_switch(Some(&"disable-breakpad".into()));
			cmd.append_switch_with_value(Some(&"disable-blink-features".into()), Some(&"WebBluetooth,WebUSB,Serial".into()));

			let extra_disabled_features = ["OptimizationHints", "OnDeviceModelService", "TranslateUI"];
			let disabled_features_switch = Some(&"disable-features".into());
			let mut disabled_features: Vec<String> = CefString::from(&cmd.switch_value(disabled_features_switch))
				.to_string()
				.split(',')
				.filter(|feature| !feature.is_empty())
				.map(ToOwned::to_owned)
				.collect();
			disabled_features.extend(extra_disabled_features.into_iter().map(ToOwned::to_owned));
			cmd.append_switch_with_value(disabled_features_switch, Some(&disabled_features.join(",").as_str().into()));

			if self.accelerated_paint {
				cmd.append_switch(Some(&"enable-gpu".into()));
				cmd.append_switch(Some(&"enable-gpu-compositing".into()));
				cmd.append_switch(Some(&"enable-begin-frame-scheduling".into()));
				cmd.append_switch(Some(&"off-screen-rendering-enabled".into()));
				cmd.append_switch(Some(&"enable-accelerated-2d-canvas".into()));

				#[cfg(target_os = "linux")]
				{
					cmd.append_switch_with_value(Some(&"use-angle".into()), Some(&"gl-egl".into()));

					let use_wayland = std::env::var("WAYLAND_DISPLAY")
						.ok()
						.filter(|var| !var.is_empty())
						.or_else(|| std::env::var("WAYLAND_SOCKET").ok())
						.filter(|var| !var.is_empty())
						.is_some();
					if use_wayland {
						cmd.append_switch_with_value(Some(&"ozone-platform".into()), Some(&"wayland".into()));
					}
				}
			} else {
				cmd.append_switch(Some(&"disable-gpu".into()));
				cmd.append_switch(Some(&"disable-gpu-compositing".into()));
			}

			#[cfg(target_os = "macos")]
			{
				// Hide user prompt asking for keychain access
				cmd.append_switch(Some(&"use-mock-keychain".into()));
			}

			// Enable browser debugging via environment variable
			if let Some(env) = std::env::var("GRAPHITE_BROWSER_DEBUG_PORT").ok()
				&& let Some(port) = env.parse::<u16>().ok()
			{
				cmd.append_switch_with_value(Some(&"remote-debugging-port".into()), Some(&port.to_string().as_str().into()));
				cmd.append_switch_with_value(Some(&"remote-allow-origins".into()), Some(&"*".into()));
			}
		}
	}

	fn get_raw(&self) -> *mut _cef_app_t {
		self.object.cast()
	}
}

impl<H: CefEventHandler> Clone for BrowserProcessAppImpl<H> {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			event_handler: self.event_handler.duplicate(),
			accelerated_paint: self.accelerated_paint,
		}
	}
}
impl<H: CefEventHandler> Rc for BrowserProcessAppImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapApp for BrowserProcessAppImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_app_t, Self>) {
		self.object = object;
	}
}
