use crate::messages::input_mapper::key_mapping::MappingVariant;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, specta::Type)]
pub struct PreferencesMessageHandler {
	pub imaginate_server_hostname: String,
	pub imaginate_refresh_frequency: f64,
	pub zoom_with_scroll: bool,
}

impl Default for PreferencesMessageHandler {
	fn default() -> Self {
		Self {
			imaginate_server_hostname: "http://localhost:7860/".into(),
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

					if self.imaginate_server_hostname != Self::default().imaginate_server_hostname {
						responses.add(PortfolioMessage::ImaginateCheckServerStatus);
					}
				}
			}
			PreferencesMessage::ResetToDefaults => {
				refresh_dialog(responses);

				*self = Self::default()
			}

			PreferencesMessage::ImaginateRefreshFrequency { seconds } => {
				self.imaginate_refresh_frequency = seconds;
				responses.add(PortfolioMessage::ImaginateCheckServerStatus);
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
				responses.add(PortfolioMessage::ImaginateCheckServerStatus);
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
