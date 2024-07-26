use crate::messages::prelude::*;

#[impl_message(Message, Preferences)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PreferencesMessage {
	Load { preferences: String },
	ResetToDefaults,

	ImaginateRefreshFrequency { seconds: f64 },
	UseVello { use_vello: bool },
	ImaginateServerHostname { hostname: String },
	ModifyLayout { zoom_with_scroll: bool },
}
