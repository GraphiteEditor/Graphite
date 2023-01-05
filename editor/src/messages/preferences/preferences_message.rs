use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, Preferences)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum PreferencesMessage {
	Load { preferences: String },
	ResetToDefaults,

	ImaginateRefreshFrequency { seconds: f64 },
	ImaginateServerHostname { hostname: String },
}
