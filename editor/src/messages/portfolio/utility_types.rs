use graphene::layers::text_layer::FontCache;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct PersistentData {
	pub font_cache: FontCache,
	pub imaginate_server_status: ImaginateServerStatus,
}

impl Default for PersistentData {
	fn default() -> Self {
		Self {
			font_cache: Default::default(),
			imaginate_server_status: ImaginateServerStatus::Unknown,
		}
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub enum ImaginateServerStatus {
	#[default]
	Unknown,
	Checking,
	Unavailable,
	Connected,
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub enum Platform {
	#[default]
	Unknown,
	Windows,
	Mac,
	Linux,
}

impl Platform {
	pub fn as_keyboard_platform_layout(&self) -> KeyboardPlatformLayout {
		match self {
			Platform::Mac => KeyboardPlatformLayout::Mac,
			Platform::Unknown => {
				warn!("The platform has not been set, remember to send `GlobalsMessage::SetPlatform` during editor initialization.");
				KeyboardPlatformLayout::Standard
			}
			_ => KeyboardPlatformLayout::Standard,
		}
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub enum KeyboardPlatformLayout {
	/// Standard keyboard mapping used by Windows and Linux
	#[default]
	Standard,
	/// Keyboard mapping used by Macs where Command is sometimes used in favor of Control
	Mac,
}
