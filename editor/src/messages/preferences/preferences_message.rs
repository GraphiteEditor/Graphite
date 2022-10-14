use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, Preferences)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PreferencesMessage {
	Load { preferences: String },
	ResetToDefaults,

	AiArtistRefreshFrequency { seconds: f64 },
	AiArtistServerHostname { hostname: String },
}
