use crate::messages::input_mapper::MappingVariant;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, specta::Type)]
pub struct PreferencesMessageHandler {
	pub imaginate_server_hostname: String,
	pub imaginate_refresh_frequency: f64,
	pub scroll_as_zoom: bool,
}

impl Default for PreferencesMessageHandler {
	fn default() -> Self {
		Self {
			imaginate_server_hostname: "http://localhost:7860/".into(),
			imaginate_refresh_frequency: 1.,
			scroll_as_zoom: matches!(MappingVariant::default(), MappingVariant::ScrollAsZoom),
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

					if self.imaginate_server_hostname != Self::default().imaginate_server_hostname {
						responses.push_back(PortfolioMessage::ImaginateCheckServerStatus.into());
					}
				}
			}
			PreferencesMessage::ResetToDefaults => {
				refresh_dialog(responses);

				*self = Self::default()
			}

			PreferencesMessage::ImaginateRefreshFrequency { seconds } => {
				self.imaginate_refresh_frequency = seconds;
				responses.push_back(PortfolioMessage::ImaginateCheckServerStatus.into());
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
				responses.push_back(PortfolioMessage::ImaginateCheckServerStatus.into());
			}
			PreferencesMessage::ModifyLayout { scroll_as_zoom } => {
				self.scroll_as_zoom = scroll_as_zoom;

				let variant = match scroll_as_zoom {
					false => MappingVariant::Default,
					true => MappingVariant::ScrollAsZoom,
				};
				responses.push_back(KeyMappingMessage::ModifyMapping(variant).into());
				responses.push_back(FrontendMessage::UpdateScrollAsZoom { scroll_as_zoom }.into());
			}
		}

		responses.push_back(FrontendMessage::TriggerSavePreferences { preferences: self.clone() }.into());
	}

	advertise_actions!(PreferencesMessageDiscriminant;
	);
}

fn refresh_dialog(responses: &mut VecDeque<Message>) {
	responses.push_back(
		DialogMessage::CloseDialogAndThen {
			followups: vec![DialogMessage::RequestPreferencesDialog.into()],
		}
		.into(),
	);
}
