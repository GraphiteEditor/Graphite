use crate::consts::VIEWPORT_ZOOM_WHEEL_RATE;
use crate::messages::input_mapper::key_mapping::MappingVariant;
use crate::messages::portfolio::document::utility_types::wires::GraphWireStyle;
use crate::messages::preferences::SelectionMode;
use crate::messages::prelude::*;
use graph_craft::wasm_application_io::EditorPreferences;

#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize, specta::Type, ExtractField)]
pub struct PreferencesMessageHandler {
	pub selection_mode: SelectionMode,
	pub zoom_with_scroll: bool,
	pub use_vello: bool,
	pub vector_meshes: bool,
	pub graph_wire_style: GraphWireStyle,
	pub viewport_zoom_wheel_rate: f64,
}

impl PreferencesMessageHandler {
	pub fn get_selection_mode(&self) -> SelectionMode {
		self.selection_mode
	}

	pub fn editor_preferences(&self) -> EditorPreferences {
		EditorPreferences {
			use_vello: self.use_vello && self.supports_wgpu(),
		}
	}

	pub fn supports_wgpu(&self) -> bool {
		graph_craft::wasm_application_io::wgpu_available().unwrap_or_default()
	}
}

impl Default for PreferencesMessageHandler {
	fn default() -> Self {
		Self {
			selection_mode: SelectionMode::Touched,
			zoom_with_scroll: matches!(MappingVariant::default(), MappingVariant::ZoomWithScroll),
			use_vello: EditorPreferences::default().use_vello,
			vector_meshes: false,
			graph_wire_style: GraphWireStyle::default(),
			viewport_zoom_wheel_rate: VIEWPORT_ZOOM_WHEEL_RATE,
		}
	}
}

#[message_handler_data]
impl MessageHandler<PreferencesMessage, ()> for PreferencesMessageHandler {
	fn process_message(&mut self, message: PreferencesMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			// Management messages
			PreferencesMessage::Load { preferences } => {
				if let Some(preferences) = preferences {
					*self = preferences;
				}

				responses.add(PortfolioMessage::EditorPreferences);
				responses.add(PortfolioMessage::UpdateVelloPreference);
				responses.add(PreferencesMessage::ModifyLayout {
					zoom_with_scroll: self.zoom_with_scroll,
				});
			}
			PreferencesMessage::ResetToDefaults => {
				refresh_dialog(responses);
				responses.add(KeyMappingMessage::ModifyMapping { mapping: MappingVariant::Default });

				*self = Self::default()
			}

			// Per-preference messages
			PreferencesMessage::UseVello { use_vello } => {
				self.use_vello = use_vello;
				responses.add(PortfolioMessage::UpdateVelloPreference);
				responses.add(PortfolioMessage::EditorPreferences);
			}
			PreferencesMessage::VectorMeshes { enabled } => {
				self.vector_meshes = enabled;
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
		}

		responses.add(FrontendMessage::TriggerSavePreferences { preferences: self.clone() });
	}

	advertise_actions!(PreferencesMessageDiscriminant;
	);
}

fn refresh_dialog(responses: &mut VecDeque<Message>) {
	responses.add(DialogMessage::CloseDialogAndThen {
		followups: vec![DialogMessage::RequestPreferencesDialog.into()],
	});
}
