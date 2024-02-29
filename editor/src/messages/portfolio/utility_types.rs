use graphene_std::{imaginate::ImaginatePersistentData, text::FontCache};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
pub struct PersistentData {
	pub font_cache: FontCache,
	pub imaginate: ImaginatePersistentData,
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
			Platform::Windows | Platform::Linux => KeyboardPlatformLayout::Standard,
			Platform::Unknown => {
				warn!("The platform has not been set, remember to send `GlobalsMessage::SetPlatform` during editor initialization.");
				KeyboardPlatformLayout::Standard
			}
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
