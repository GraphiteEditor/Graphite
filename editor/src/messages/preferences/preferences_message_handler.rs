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
	/// Who this person is across documents, drawn once and kept.
	/// Stored as a decimal string, since the browser's JSON loses precision on a 64-bit number.
	#[serde(with = "u64_as_string")]
	#[cfg_attr(feature = "wasm", tsify(type = "string"))]
	pub user_id: u64,
	/// The display name peers see in a live session. Empty means unnamed.
	pub user_name: String,
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
			user_id: crate::application::generate_uuid(),
			user_name: String::new(),
			show_remote_cursors: true,
			#[cfg(target_os = "macos")]
			vsync: false,
		}
	}
}

impl PreferencesMessageHandler {
	/// The preferences stored as JSON, or the defaults when none are stored or they do not parse.
	pub fn from_stored(stored: Option<&str>) -> Self {
		let Some(stored) = stored else { return Self::default() };
		serde_json::from_str(stored).unwrap_or_else(|error| {
			log::error!("Failed to deserialize preferences: {error}");
			Self::default()
		})
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
				let preferences = Self {
					user_id: self.user_id,
					..Self::default()
				};
				responses.add(PreferencesMessage::Load { preferences });
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

/// A `u64` written as a decimal string, read from a string or a number.
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_first_launch_without_stored_preferences_has_a_user_id() {
		assert_ne!(PreferencesMessageHandler::default().user_id, 0);
		let stored: PreferencesMessageHandler = serde_json::from_str("{}").unwrap();
		assert_ne!(stored.user_id, 0);
	}

	#[test]
	fn missing_or_unreadable_stored_preferences_load_as_defaults_and_are_persisted() {
		for stored in [None, Some("not json")] {
			let mut preferences = PreferencesMessageHandler::default();
			let loaded = PreferencesMessageHandler::from_stored(stored);
			assert_ne!(loaded.user_id, 0);

			let mut responses = VecDeque::new();
			let tool_message_handler = ToolMessageHandler::default();
			let context = PreferencesMessageContext {
				tool_message_handler: &tool_message_handler,
			};
			preferences.process_message(PreferencesMessage::Load { preferences: loaded.clone() }, &mut responses, context);
			let persisted = responses.iter().find_map(|message| match message {
				Message::Frontend(FrontendMessage::TriggerUpdatePreferences { preferences }) => Some(preferences),
				_ => None,
			});
			assert_eq!(persisted.map(|persisted| persisted.user_id), Some(loaded.user_id));
		}
	}

	#[test]
	fn resetting_the_preferences_keeps_the_user_id() {
		let mut preferences = PreferencesMessageHandler::default();
		let tool_message_handler = ToolMessageHandler::default();
		let mut responses = VecDeque::new();
		preferences.process_message(
			PreferencesMessage::ResetToDefaults,
			&mut responses,
			PreferencesMessageContext {
				tool_message_handler: &tool_message_handler,
			},
		);
		let reset = responses.iter().find_map(|message| match message {
			Message::Preferences(PreferencesMessage::Load { preferences }) => Some(preferences),
			_ => None,
		});
		assert_eq!(reset.map(|reset| reset.user_id), Some(preferences.user_id));
	}

	#[test]
	fn the_user_id_round_trips_as_a_string() {
		let preferences = PreferencesMessageHandler {
			user_id: u64::MAX,
			..Default::default()
		};
		let json = serde_json::to_value(&preferences).unwrap();
		assert_eq!(json["user_id"], serde_json::Value::from(u64::MAX.to_string()));
		let read: PreferencesMessageHandler = serde_json::from_value(json).unwrap();
		assert_eq!(read.user_id, u64::MAX);
	}
}
