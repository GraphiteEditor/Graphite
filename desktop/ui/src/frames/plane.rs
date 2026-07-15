#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::*;

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub(crate) use win::*;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
pub(crate) use mac::*;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) enum RecvResult {
	Frame(WireFrame),
	WouldBlock,
	#[cfg_attr(target_os = "macos", allow(dead_code))]
	Closed,
}

/// Decode the wire representation of `cef_color_type_t` (its `u32` discriminant),
/// logging unknown discriminants.
fn wire_color_type(format: u32) -> Option<cef::sys::cef_color_type_t> {
	match format {
		0 => Some(cef::sys::cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888),
		1 => Some(cef::sys::cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888),
		_ => {
			tracing::error!("Unknown color type {format} in accelerated frame");
			None
		}
	}
}
