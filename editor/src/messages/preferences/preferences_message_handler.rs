use crate::messages::input_mapper::key_mapping::MappingVariant;
use crate::messages::prelude::*;
use graph_craft::imaginate_input::ImaginatePreferences;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, specta::Type)]
pub struct PreferencesMessageHandler {
	pub imaginate_server_hostname: String,
	pub imaginate_refresh_frequency: f64,
	pub zoom_with_scroll: bool,
}

impl PreferencesMessageHandler {
	pub fn get_imaginate_preferences(&self) -> ImaginatePreferences {
		ImaginatePreferences {
			host_name: self.imaginate_server_hostname.clone(),
		}
	}
}

impl Default for PreferencesMessageHandler {
	fn default() -> Self {
		let ImaginatePreferences { host_name } = Default::default();
		Self {
			imaginate_server_hostname: host_name,
			imaginate_refresh_frequency: 1.,
			zoom_with_scroll: matches!(MappingVariant::default(), MappingVariant::ZoomWithScroll),
		}
	}
}

impl MessageHandler<PreferencesMessage, ()> for PreferencesMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PreferencesMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			PreferencesMessage::Load { preferences } => {
				if let Ok(deserialized_preferences) = serde_json::from_str::<PreferencesMessageHandler>(&preferences) {
					*self = deserialized_preferences;

					responses.add(PortfolioMessage::ImaginateServerHostname);
					responses.add(PortfolioMessage::ImaginateCheckServerStatus);
					responses.add(PortfolioMessage::ImaginatePreferences);
				}
			}
			PreferencesMessage::ResetToDefaults => {
				refresh_dialog(responses);
				responses.add(KeyMappingMessage::ModifyMapping(MappingVariant::Default));

				*self = Self::default()
			}

			PreferencesMessage::ImaginateRefreshFrequency { seconds } => {
				self.imaginate_refresh_frequency = seconds;
				responses.add(PortfolioMessage::ImaginateCheckServerStatus);
				responses.add(PortfolioMessage::ImaginatePreferences);
			}
			PreferencesMessage::ImaginateServerHostname { hostname } => {
				let initial = hostname.clone();
				let has_protocol = hostname.starts_with("http://") || hostname.starts_with("https://");
				let hostname = if has_protocol { hostname } else { "http://".to_string() + &hostname };
				let hostname = if hostname.ends_with('/') { hostname } else { hostname + "/" };

				if hostname != initial {
					refresh_dialog(responses);
				}

				self.imaginate_server_hostname = hostname;
				responses.add(PortfolioMessage::ImaginateServerHostname);
				responses.add(PortfolioMessage::ImaginateCheckServerStatus);
				responses.add(PortfolioMessage::ImaginatePreferences);
			}
			PreferencesMessage::ModifyLayout { zoom_with_scroll } => {
				self.zoom_with_scroll = zoom_with_scroll;

				let variant = match zoom_with_scroll {
					false => MappingVariant::Default,
					true => MappingVariant::ZoomWithScroll,
				};
				responses.add(KeyMappingMessage::ModifyMapping(variant));
				responses.add(FrontendMessage::UpdateZoomWithScroll { zoom_with_scroll });
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
