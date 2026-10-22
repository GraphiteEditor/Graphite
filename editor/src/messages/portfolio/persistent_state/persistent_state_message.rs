use crate::messages::frontend::utility_types::PersistedState;
use crate::messages::prelude::*;

#[impl_message(Message, PortfolioMessage, PersistentState)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PersistentStateMessage {
	ReadState,
	WriteState,
	LoadState { state: PersistedState },
}
