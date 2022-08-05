pub use super::layer_panel::{LayerMetadata, LayerPanelEntry};

use graphene::document::Document as GrapheneDocument;
use graphene::LayerId;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

pub type DocumentSave = (GrapheneDocument, HashMap<Vec<LayerId>, LayerMetadata>);

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
	Average,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TargetDocument {
	Artboard,
	Artwork,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DocumentMode {
	DesignMode,
	SelectMode,
	GuideMode,
}

impl fmt::Display for DocumentMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let text = match self {
			DocumentMode::DesignMode => "Design Mode".to_string(),
			DocumentMode::SelectMode => "Select Mode".to_string(),
			DocumentMode::GuideMode => "Guide Mode".to_string(),
		};
		write!(f, "{}", text)
	}
}

impl DocumentMode {
	pub fn icon_name(&self) -> String {
		match self {
			DocumentMode::DesignMode => "ViewportDesignMode".to_string(),
			DocumentMode::SelectMode => "ViewportSelectMode".to_string(),
			DocumentMode::GuideMode => "ViewportGuideMode".to_string(),
		}
	}
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
				log::warn!("The platform has not been set, remember to send `PortfolioMessage::SetPlatform` during editor initialization.");
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
