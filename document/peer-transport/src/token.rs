use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use std::fmt;
use std::str::FromStr;

const PREFIX: &str = "graphite-";

/// Session ID, used as the matchbox room name. Entropy is supplied by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionToken(pub [u8; 16]);

impl SessionToken {
	pub fn signaling_url(&self, signaling_server: &str) -> String {
		format!("{}/{}", signaling_server.trim_end_matches('/'), self)
	}

	/// The token every copy of a document shares, so a copy edited apart reconnects by opening the file. The room is
	/// as unguessable as the random document id; the mixing spreads it over the token's width.
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

	/// Parses the bare token or a share link carrying it as the `session` query parameter.
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let s = s.trim();
		let query = s.split(['#', ' ']).next().and_then(|url| url.split_once('?')).map(|(_, query)| query);
		let s = query.and_then(|query| query.split('&').find_map(|parameter| parameter.strip_prefix("session="))).unwrap_or(s);
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
	fn parses_the_token_out_of_a_share_link() {
		let token = SessionToken([7; 16]);
		let link = format!(" https://editor.graphite.rs/?session={token}#top ");
		assert_eq!(link.parse::<SessionToken>().unwrap(), token);
		assert_eq!(format!("http://localhost:8080/?debug=1&session={token}").parse::<SessionToken>().unwrap(), token);
		let other = SessionToken([9; 16]);
		assert_eq!(format!("https://editor.graphite.rs/?next=session={other}&session={token}").parse::<SessionToken>().unwrap(), token);
		assert!(format!("https://editor.graphite.rs/?not_session={token}").parse::<SessionToken>().is_err());
	}

	#[test]
	fn rejects_wrong_length_or_missing_prefix() {
		assert!("graphite-AAAA".parse::<SessionToken>().is_err());
		assert!("BwcHBwcHBwcHBwcHBwcHBw".parse::<SessionToken>().is_err());
	}

	#[test]
	fn a_document_id_derives_one_token() {
		assert_eq!(SessionToken::for_document(7), SessionToken::for_document(7));
		assert_ne!(SessionToken::for_document(7), SessionToken::for_document(8));
		let text = SessionToken::for_document(7).to_string();
		assert_eq!(text.parse::<SessionToken>().expect("round trip"), SessionToken::for_document(7));
	}
}
