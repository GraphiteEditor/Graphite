use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct PreferencesMessageHandler {
	pub ai_artist_server_hostname: String,
	pub ai_artist_refresh_frequency: f64,
}

impl Default for PreferencesMessageHandler {
	fn default() -> Self {
		Self {
			ai_artist_server_hostname: "http://localhost:7860/".into(),
			ai_artist_refresh_frequency: 1.,
		}
	}
}

impl MessageHandler<PreferencesMessage, ()> for PreferencesMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PreferencesMessage, _data: (), responses: &mut VecDeque<Message>) {
		match message {
			PreferencesMessage::Load { preferences } => {
				if let Ok(deserialized_preferences) = serde_json::from_str::<PreferencesMessageHandler>(&preferences) {
					*self = deserialized_preferences;

					if self.ai_artist_server_hostname != Self::default().ai_artist_server_hostname {
						responses.push_back(PortfolioMessage::AiArtistCheckServerStatus.into());
					}
				}
			}
			PreferencesMessage::ResetToDefaults => {
				refresh_dialog(responses);

				*self = Self::default()
			}

			PreferencesMessage::AiArtistRefreshFrequency { seconds } => {
				self.ai_artist_refresh_frequency = seconds;
				responses.push_back(PortfolioMessage::AiArtistCheckServerStatus.into());
			}
			PreferencesMessage::AiArtistServerHostname { hostname } => {
				let initial = hostname.clone();
				let has_protocol = hostname.starts_with("http://") || hostname.starts_with("https://");
				let hostname = if has_protocol { hostname } else { "http://".to_string() + &hostname };
				let hostname = if hostname.ends_with('/') { hostname } else { hostname + "/" };

				if hostname != initial {
					refresh_dialog(responses);
				}

				self.ai_artist_server_hostname = hostname;
				responses.push_back(PortfolioMessage::AiArtistCheckServerStatus.into());
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
