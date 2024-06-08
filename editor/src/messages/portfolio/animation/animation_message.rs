use crate::messages::prelude::*;

#[impl_message(Message, PortfolioMessage, Animation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AnimationMessage {
	NextFrame,
	Play,
	Pause,
	Restart,
}
