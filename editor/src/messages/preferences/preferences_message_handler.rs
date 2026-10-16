use crate::consts::{UI_SCALE_DEFAULT, VIEWPORT_ZOOM_WHEEL_RATE};
use crate::messages::input_mapper::key_mapping::MappingVariant;
use crate::messages::portfolio::document::utility_types::wires::GraphWireStyle;
use crate::messages::preferences::SelectionMode;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::ToolType;

#[derive(ExtractField)]
pub struct PreferencesMessageContext<'a> {
	pub tool_message_handler: &'a ToolMessageHandler,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize, ExtractField)]
#[serde(default)]
pub struct PreferencesMessageHandler {
	pub selection_mode: SelectionMode,
	pub zoom_with_scroll: bool,
	pub brush_tool: bool,
	pub graph_wire_style: GraphWireStyle,
	pub viewport_zoom_wheel_rate: f64,
	pub ui_scale: f64,
	pub disable_ui_acceleration: bool,
	pub validate_storage_round_trip: bool,
	pub save_as_gdd: bool,
	pub show_storage_preferences: bool,
	/// Who this person is across documents and devices, drawn once and kept; `0` until then.
	/// Kept as a decimal string in the preferences file: a 64-bit number does not survive the browser's JSON store.
	#[serde(with = "u64_as_string")]
	#[cfg_attr(feature = "wasm", tsify(type = "string"))]
	pub user_id: u64,
	/// The display name peers see in a live session; empty means unnamed.
	pub user_name: String,
	/// Draw the other peers' pointers over a shared document.
	pub show_remote_cursors: bool,
	#[cfg(target_os = "macos")]
	pub vsync: bool,
}

impl PreferencesMessageHandler {
	pub fn preferences_requiring_restart(&self, other: &Self) -> Vec<String> {
		let requiring_restart: Vec<String> = Vec::new();
		let _ = other;

		requiring_restart
	}

	pub fn get_selection_mode(&self) -> SelectionMode {
		self.selection_mode
	}
}

impl Default for PreferencesMessageHandler {
	fn default() -> Self {
		Self {
			selection_mode: SelectionMode::Touched,
			zoom_with_scroll: matches!(MappingVariant::default(), MappingVariant::ZoomWithScroll),
			brush_tool: false,
			graph_wire_style: GraphWireStyle::default(),
			viewport_zoom_wheel_rate: VIEWPORT_ZOOM_WHEEL_RATE,
			ui_scale: UI_SCALE_DEFAULT,
			disable_ui_acceleration: true, // TODO: Set this back to false once we have ui acceleration working more reliably
			validate_storage_round_trip: false,
			save_as_gdd: false,
			show_storage_preferences: false,
			user_id: 0,
			user_name: String::new(),
			show_remote_cursors: true,
			#[cfg(target_os = "macos")]
			vsync: false,
		}
	}
}

#[message_handler_data]
impl MessageHandler<PreferencesMessage, PreferencesMessageContext<'_>> for PreferencesMessageHandler {
	fn process_message(&mut self, message: PreferencesMessage, responses: &mut VecDeque<Message>, context: PreferencesMessageContext) {
		let PreferencesMessageContext { tool_message_handler } = context;

		match message {
			// Management messages
			PreferencesMessage::Load { preferences } => {
				*self = preferences;
				if self.user_id == 0 {
					self.user_id = crate::application::generate_uuid();
				}

				responses.add(PreferencesMessage::ModifyLayout {
					zoom_with_scroll: self.zoom_with_scroll,
				});
				responses.add(FrontendMessage::UpdateUIScale { scale: self.ui_scale });
				responses.add(ToolMessage::RefreshToolShelf);
			}
			PreferencesMessage::ResetToDefaults => {
				responses.add(PreferencesMessage::Load { preferences: Self::default() });
				responses.add(DialogMessage::RequestPreferencesDialog);
			}

			// Per-preference messages
			PreferencesMessage::BrushTool { enabled } => {
				self.brush_tool = enabled;

				if !enabled && tool_message_handler.tool_state.tool_data.active_tool_type == ToolType::Brush {
					responses.add(ToolMessage::ActivateToolSelect);
				}

				responses.add(ToolMessage::RefreshToolShelf);
			}
			PreferencesMessage::ModifyLayout { zoom_with_scroll } => {
				self.zoom_with_scroll = zoom_with_scroll;

				let variant = if zoom_with_scroll { MappingVariant::ZoomWithScroll } else { MappingVariant::Default };
				responses.add(KeyMappingMessage::ModifyMapping { mapping: variant });
			}
			PreferencesMessage::SelectionMode { selection_mode } => {
				self.selection_mode = selection_mode;
			}
			PreferencesMessage::GraphWireStyle { style } => {
				self.graph_wire_style = style;
				responses.add(NodeGraphMessage::UnloadWires);
				responses.add(NodeGraphMessage::SendWires);
			}
			PreferencesMessage::ViewportZoomWheelRate { rate } => {
				self.viewport_zoom_wheel_rate = rate;
			}
			PreferencesMessage::UIScale { scale } => {
				self.ui_scale = scale;
				responses.add(FrontendMessage::UpdateUIScale { scale: self.ui_scale });
			}
			PreferencesMessage::DisableUIAcceleration { disable_ui_acceleration } => {
				self.disable_ui_acceleration = disable_ui_acceleration;
			}
			PreferencesMessage::ValidateStorageRoundTrip { enabled } => {
				self.validate_storage_round_trip = enabled;
			}
			PreferencesMessage::SaveAsGdd { enabled } => {
				self.save_as_gdd = enabled;
			}
			PreferencesMessage::ToggleShowStoragePreferences => {
				self.show_storage_preferences = !self.show_storage_preferences;
				responses.add(MenuBarMessage::SendLayout);
			}
			PreferencesMessage::UserName { name } => {
				self.user_name = name.trim().to_string();
				responses.add(SyncMessage::RefreshPanel);
			}
			PreferencesMessage::ShowRemoteCursors { enabled } => {
				self.show_remote_cursors = enabled;
				responses.add(SyncMessage::RefreshPanel);
				responses.add(OverlaysMessage::Draw);
			}
			#[cfg(target_os = "macos")]
			PreferencesMessage::VSync { vsync } => {
				self.vsync = vsync;
			}
		}

		responses.add(FrontendMessage::TriggerUpdatePreferences { preferences: self.clone() });
	}

	advertise_actions!(PreferencesMessageDiscriminant;
	);
}

/// A `u64` written as a decimal string, since the browser's JSON loses precision on a number this wide and cannot write a
/// BigInt. Reads a number too.
mod u64_as_string {
	use serde::{Deserialize, Deserializer, Serializer};

	pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_str(&value.to_string())
	}

	pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
		#[derive(Deserialize)]
		#[serde(untagged)]
		enum Stored {
			Text(String),
			Number(u64),
		}
		match Stored::deserialize(deserializer)? {
			Stored::Text(text) => text.parse().map_err(serde::de::Error::custom),
			Stored::Number(number) => Ok(number),
		}
	}
}
