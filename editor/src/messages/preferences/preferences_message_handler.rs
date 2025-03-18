use crate::messages::input_mapper::key_mapping::MappingVariant;
use crate::messages::portfolio::document::node_graph::utility_types::GraphWireStyle;
use crate::messages::preferences::SelectionMode;
use crate::messages::prelude::*;
use graph_craft::wasm_application_io::EditorPreferences;

#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PreferencesMessageHandler {
	pub imaginate_server_hostname: String,
	pub imaginate_refresh_frequency: f64,
	pub selection_mode: SelectionMode,
	pub zoom_with_scroll: bool,
	pub use_vello: bool,
	pub vector_meshes: bool,
	pub graph_wire_style: GraphWireStyle,
}

impl PreferencesMessageHandler {
	pub fn get_selection_mode(&self) -> SelectionMode {
		self.selection_mode
	}

	pub fn editor_preferences(&self) -> EditorPreferences {
		EditorPreferences {
			imaginate_hostname: self.imaginate_server_hostname.clone(),
			use_vello: self.use_vello && self.supports_wgpu(),
		}
	}

	pub fn supports_wgpu(&self) -> bool {
		graph_craft::wasm_application_io::wgpu_available().unwrap_or_default()
	}
}

impl Default for PreferencesMessageHandler {
	fn default() -> Self {
		let EditorPreferences {
			imaginate_hostname: host_name,
			use_vello,
		} = Default::default();

		Self {
			imaginate_server_hostname: host_name,
			imaginate_refresh_frequency: 1.,
			selection_mode: SelectionMode::Touched,
			zoom_with_scroll: matches!(MappingVariant::default(), MappingVariant::ZoomWithScroll),
			use_vello,
			vector_meshes: false,
			graph_wire_style: GraphWireStyle::default(),
		}
	}
}

impl MessageHandler<PreferencesMessage, ()> for PreferencesMessageHandler {
	fn process_message(&mut self, message: PreferencesMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			// Management messages
			PreferencesMessage::Load { preferences } => {
				if let Ok(deserialized_preferences) = serde_json::from_str::<PreferencesMessageHandler>(&preferences) {
					*self = deserialized_preferences;

					// TODO: Reenable when Imaginate is restored
					// responses.add(PortfolioMessage::ImaginateServerHostname);
					// responses.add(PortfolioMessage::ImaginateCheckServerStatus);

					responses.add(PortfolioMessage::EditorPreferences);
					responses.add(PortfolioMessage::UpdateVelloPreference);
					responses.add(PreferencesMessage::ModifyLayout {
						zoom_with_scroll: self.zoom_with_scroll,
					});
				}
			}
			PreferencesMessage::ResetToDefaults => {
				refresh_dialog(responses);
				responses.add(KeyMappingMessage::ModifyMapping(MappingVariant::Default));

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

				let variant = if zoom_with_scroll { MappingVariant::Default } else { MappingVariant::ZoomWithScroll };
				responses.add(KeyMappingMessage::ModifyMapping(variant));
			}
			PreferencesMessage::SelectionMode { selection_mode } => {
				self.selection_mode = selection_mode;
			}
			PreferencesMessage::GraphWireStyle { style } => {
				self.graph_wire_style = style;
				responses.add(NodeGraphMessage::SendGraph);
			}
		}
		// TODO: Reenable when Imaginate is restored (and move back up one line since the auto-formatter doesn't like it in that block)
		// PreferencesMessage::ImaginateRefreshFrequency { seconds } => {
		// 	self.imaginate_refresh_frequency = seconds;
		// 	responses.add(PortfolioMessage::ImaginateCheckServerStatus);
		// 	responses.add(PortfolioMessage::EditorPreferences);
		// }
		// PreferencesMessage::ImaginateServerHostname { hostname } => {
		// 	let initial = hostname.clone();
		// 	let has_protocol = hostname.starts_with("http://") || hostname.starts_with("https://");
		// 	let hostname = if has_protocol { hostname } else { "http://".to_string() + &hostname };
		// 	let hostname = if hostname.ends_with('/') { hostname } else { hostname + "/" };

		// 	if hostname != initial {
		// 		refresh_dialog(responses);
		// 	}

		//	self.imaginate_server_hostname = hostname;
		//	responses.add(PortfolioMessage::ImaginateServerHostname);
		//	responses.add(PortfolioMessage::ImaginateCheckServerStatus);
		//	responses.add(PortfolioMessage::EditorPreferences);
		//}

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
