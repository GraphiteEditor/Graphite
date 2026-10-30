//! Line commands for driving a live document from stdin:
//!
//! ```text
//! set <key> <json>   stage a document attribute as a hot op
//! retire             promote pending hot ops (host only)
//! show               print document attributes and networks
//! history            print retired deltas
//! quit
//! ```

use document_graph_storage::{AttributeDelta, RegistryDelta, Rev};
use document_live::{Event, LiveDocument};

pub fn report(event: &Event) {
	match event {
		Event::PeerJoined { peer, user } => println!("peer {peer:?} joined as {user:?}"),
		Event::PeerLeft { peer } => println!("peer {peer:?} left"),
		Event::RoleChanged { role } => println!("role is now {role:?}"),
		Event::ProfileChanged { peer } => println!("peer {peer:?} announced a name"),
		Event::CursorMoved { .. } => {}
		Event::MetadataChanged => println!("the record of the document's users changed"),
		Event::Synced => println!("synced"),
		Event::Changed => println!("document changed"),
		Event::ResourceReceived { hash, bytes } => println!("received resource {hash} ({} bytes)", bytes.len()),
		Event::ResourceRequested { hash, .. } => println!("peer asked for resource {hash}"),
	}
}

/// Returns `false` to quit.
pub fn run_command(live: &mut LiveDocument, line: &str) -> bool {
	match split_command(line) {
		(Some("set"), Some(key), Some(value)) => {
			let Ok(value) = serde_json::from_str(value) else {
				println!("value must be JSON");
				return true;
			};
			let delta = AttributeDelta {
				key: key.to_string(),
				value: Some(value),
			};
			if let Err(error) = live.document_mut().stage_ops([RegistryDelta::ChangeDocumentAttribute { delta }]) {
				println!("stage failed: {error}");
			}
		}
		(Some("retire"), ..) => match live.document_mut().retire_pending_interaction() {
			Ok(revs) => println!("retired {} deltas", revs.len()),
			Err(error) => println!("retire failed: {error}"),
		},
		(Some("show"), ..) => {
			let registry = live.document().registry();
			for (key, value) in &registry.attributes {
				println!("{key} = {:?} @{}", value.value, value.timestamp.counter);
			}
			for id in registry.networks.keys() {
				println!("network {id:?}");
			}
			println!("{} hot ops pending", live.document().session().hot_log().len());
		}
		(Some("history"), ..) => {
			for delta in live.document().session().history() {
				println!("{} by {:?} @{} {}", short_rev(delta.id), delta.author, delta.timestamp.counter, describe(&delta.kind));
			}
		}
		(Some("quit"), ..) => return false,
		_ => println!("commands: set <key> <json>, retire, show, history, quit"),
	}
	true
}

/// The command word, the word after it, and the rest of the line with its own spacing kept, split at runs of whitespace.
fn split_command(line: &str) -> (Option<&str>, Option<&str>, Option<&str>) {
	fn next_word(text: &str) -> (Option<&str>, &str) {
		let text = text.trim_start();
		let (word, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
		((!word.is_empty()).then_some(word), rest)
	}
	let (command, rest) = next_word(line);
	let (argument, rest) = next_word(rest);
	let rest = rest.trim();
	(command, argument, (!rest.is_empty()).then_some(rest))
}

fn describe(kind: &RegistryDelta) -> String {
	match kind {
		RegistryDelta::ChangeDocumentAttribute { delta } => format!("set {} = {:?}", delta.key, delta.value),
		RegistryDelta::RegisterPeer { peer, user } => format!("register {peer:?} as {user:?}"),
		RegistryDelta::Merge { extra_parents } => format!("merge {} extra parents", extra_parents.len()),
		other => format!("{other:?}").chars().take(60).collect(),
	}
}

fn short_rev(rev: Rev) -> String {
	format!("{rev}").chars().take(8).collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn commands_split_at_any_run_of_whitespace() {
		assert_eq!(split_command("set  name\t\"a  b\" "), (Some("set"), Some("name"), Some("\"a  b\"")));
		assert_eq!(split_command("\tretire"), (Some("retire"), None, None));
		assert_eq!(split_command("   "), (None, None, None));
	}
}
