use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use std::fmt;
use std::str::FromStr;

/// Identifies the text form as a Graphite session token, like GitLab's `glpat-` tokens.
const PREFIX: &str = "graphite-";

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
		write!(f, "{PREFIX}{}", URL_SAFE_NO_PAD.encode(self.0))
	}
}

#[derive(Debug, thiserror::Error)]
#[error("invalid session token")]
pub struct InvalidSessionToken;

impl FromStr for SessionToken {
	type Err = InvalidSessionToken;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let encoded = s.strip_prefix(PREFIX).ok_or(InvalidSessionToken)?;
		let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| InvalidSessionToken)?;
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
	fn rejects_wrong_length_or_missing_prefix() {
		assert!("graphite-AAAA".parse::<SessionToken>().is_err());
		assert!("BwcHBwcHBwcHBwcHBwcHBw".parse::<SessionToken>().is_err());
	}
}

impl SessionToken {
	/// The token every copy of one document shares: derived from the document's id, so a copy edited
	/// apart reconnects to the same room as the others by opening the same file, with no link to pass
	/// around. Document ids are random 64-bit values, so the room is no more guessable than the id; the
	/// mixing only spreads the id over the token's width.
	pub fn for_document(document_id: u64) -> Self {
		fn mix(mut value: u64) -> u64 {
			value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
			value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
			value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
			value ^ (value >> 31)
		}
		let mut bytes = [0u8; 16];
		bytes[..8].copy_from_slice(&mix(document_id).to_le_bytes());
		bytes[8..].copy_from_slice(&mix(document_id ^ 0x5A5A_5A5A_5A5A_5A5A).to_le_bytes());
		Self(bytes)
	}
}

#[cfg(test)]
mod derived_token_tests {
	use super::SessionToken;

	/// Every copy of a document derives the same token, and different documents different ones.
	#[test]
	fn a_document_id_derives_one_token() {
		assert_eq!(SessionToken::for_document(7), SessionToken::for_document(7));
		assert_ne!(SessionToken::for_document(7), SessionToken::for_document(8));
		let text = SessionToken::for_document(7).to_string();
		assert_eq!(text.parse::<SessionToken>().expect("round trip"), SessionToken::for_document(7));
	}
}
