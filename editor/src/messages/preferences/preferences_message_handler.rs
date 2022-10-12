use crate::messages::prelude::*;

#[derive(Debug)]
pub struct PreferencesMessageHandler {
	pub ai_artist_server_hostname: String,
}

impl Default for PreferencesMessageHandler {
	fn default() -> Self {
		Self {
			ai_artist_server_hostname: "http://192.168.1.10:7860/".into(),
		}
	}
}

impl MessageHandler<PreferencesMessage, ()> for PreferencesMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PreferencesMessage, _data: (), responses: &mut VecDeque<Message>) {
		match message {
			PreferencesMessage::AiArtistServerHostname { hostname } => {
				self.ai_artist_server_hostname = hostname;
				responses.push_back(PortfolioMessage::AiArtistCheckServerStatus.into());
			}
		}
	}

	advertise_actions!(PreferencesMessageDiscriminant;
	);
}
