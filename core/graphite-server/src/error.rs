use std::sync::mpsc::SendError;

use crate::group;

#[allow(dead_code)]
#[derive(Debug)]
pub enum ServerError {
	Group(String),
	GroupCreation(String),
	GameCreation(std::io::Error),
	WebSocketCreation(std::io::Error),
	WebSocketError(ws::Error),
	InvalidProtocol,
	InvalidTokenFormat,
	InvalidToken(String),
	InvalidUser(usize),
	StdErr(Box<dyn std::error::Error>),
	MessageSend(SendError<group::Message>),
	FileError(std::io::Error),
}

impl std::fmt::Display for ServerError {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			ServerError::Group(e) => write!(f, "GroupError: {}", e),
			ServerError::GroupCreation(e) => write!(f, "GroupCreationError: {}", e),
			ServerError::GameCreation(e) => write!(f, "GameCreationError: {}", e),
			ServerError::WebSocketCreation(e) => write!(f, "WebSocketCreationError: {}", e),
			ServerError::WebSocketError(e) => write!(f, "WebSocketError: {}", e),
			ServerError::InvalidProtocol => write!(f, "InvalidProtocolError"),
			ServerError::InvalidTokenFormat => write!(f, "InvalidTokenFormat"),
			ServerError::InvalidToken(e) => write!(f, "InvalidTokenError: {}", e),
			ServerError::InvalidUser(e) => {
				write!(f, "Invalid User id: {}. User is not in the Game", e)
			}
			ServerError::StdErr(e) => write!(f, "StdErrorError: {}", e),
			ServerError::MessageSend(e) => write!(f, "MessageSendError: {}", e),
			ServerError::FileError(e) => write!(f, "FileError: {}", e),
		}
	}
}

macro_rules! derive_from {
	($type:ty, $kind:ident) => {
		impl From<$type> for ServerError {
			fn from(error: $type) -> Self {
				ServerError::$kind(error)
			}
		}
	};
}

derive_from!(ws::Error, WebSocketError);
derive_from!(std::io::Error, FileError);
derive_from!(Box<dyn std::error::Error>, StdErr);
derive_from!(SendError<group::Message>, MessageSend);

impl std::error::Error for ServerError {}
