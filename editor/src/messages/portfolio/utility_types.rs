use graphene_std::text::FontCache;

#[derive(Debug, Default)]
pub struct PersistentData {
	pub font_cache: FontCache,
	pub use_vello: bool,
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize)]
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

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum KeyboardPlatformLayout {
	/// Standard keyboard mapping used by Windows and Linux
	#[default]
	Standard,
	/// Keyboard mapping used by Macs where Command is sometimes used in favor of Control
	Mac,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum PanelType {
	#[default]
	Document,
	Layers,
	Properties,
	Spreadsheet,
}

impl From<String> for PanelType {
	fn from(value: String) -> Self {
		match value.as_str() {
			"Document" => PanelType::Document,
			"Layers" => PanelType::Layers,
			"Properties" => PanelType::Properties,
			"Spreadsheet" => PanelType::Spreadsheet,
			_ => panic!("Unknown panel type: {value}"),
		}
	}
}
