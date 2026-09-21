use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use std::fmt;
use std::str::FromStr;

/// Random session ID, used as the matchbox room name. Entropy is supplied by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionToken(pub [u8; 16]);

impl SessionToken {
	pub fn signaling_url(&self, signaling_server: &str) -> String {
		format!("{}/{}", signaling_server.trim_end_matches('/'), self)
	}
}

impl fmt::Display for SessionToken {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&URL_SAFE_NO_PAD.encode(self.0))
	}
}

#[derive(Debug, thiserror::Error)]
#[error("invalid session token")]
pub struct InvalidSessionToken;

impl FromStr for SessionToken {
	type Err = InvalidSessionToken;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let bytes = URL_SAFE_NO_PAD.decode(s).map_err(|_| InvalidSessionToken)?;
		let bytes = bytes.try_into().map_err(|_| InvalidSessionToken)?;
		Ok(Self(bytes))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn round_trips_through_string() {
		let token = SessionToken([7; 16]);
		assert_eq!(token.to_string().parse::<SessionToken>().unwrap(), token);
	}

	#[test]
	fn rejects_wrong_length() {
		assert!("AAAA".parse::<SessionToken>().is_err());
	}
}
