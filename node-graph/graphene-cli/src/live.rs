//! The `host`, `join` and `watch` subcommands: a memory-backed document in a collaborative session,
//! driven from stdin or shown in a window.

use crate::repl;
use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{PeerId, UserId};
use document_live::{LiveDocument, SessionToken};
use std::error::Error;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};

pub use document_live::DEFAULT_SIGNALING_SERVER;

pub async fn host(signaling: &str, document: Option<&Path>) -> Result<(), Box<dyn Error>> {
	let (peer, user) = identity()?;
	let live = match document {
		Some(path) => {
			let gdd = open_archive(path).await?;
			let token = SessionToken::for_document(gdd.manifest().document_id);
			LiveDocument::host_document(gdd, signaling, token, user, spawn_driver)?
		}
		// An empty document takes the peer id as its document id
		None => LiveDocument::host(signaling, SessionToken::for_document(peer.0), peer, user, spawn_driver).await?,
	};
	println!("hosting session {}", live.token());
	drive_from_stdin(live).await;
	Ok(())
}

pub async fn join(signaling: &str, token: SessionToken) -> Result<(), Box<dyn Error>> {
	let (peer, user) = identity()?;
	let live = LiveDocument::join(signaling, token, peer, user, spawn_driver).await?;
	println!("joining session {token}");
	drive_from_stdin(live).await;
	Ok(())
}

pub async fn watch(signaling: &str, token: SessionToken) -> Result<(), Box<dyn Error>> {
	let (peer, user) = identity()?;
	let live = LiveDocument::join(signaling, token, peer, user, spawn_driver).await?;
	crate::watch::run(live).await
}

async fn open_archive(path: &Path) -> Result<GddV1, Box<dyn Error>> {
	let archive = std::fs::read(path).map_err(|error| format!("Failed to read document {}: {error}", path.display()))?;
	let container = AnyContainer::Memory(MemoryBackend::new());
	Ok(GddV1::open_from_archive(&archive, container, GddV1Layout).await?)
}

async fn drive_from_stdin(mut live: LiveDocument) {
	let mut lines = BufReader::new(tokio::io::stdin()).lines();
	let mut tick = tokio::time::interval(Duration::from_millis(100));

	loop {
		tokio::select! {
			_ = tick.tick() => {
				for event in live.poll().await {
					repl::report(&event);
				}
			}
			line = lines.next_line() => {
				let Ok(Some(line)) = line else { break };
				if !repl::run_command(&mut live, line.trim()) {
					break;
				}
			}
		}
	}
	live.leave();
}

fn spawn_driver(driver: document_live::MessageLoopFuture) {
	tokio::spawn(driver);
}

fn identity() -> Result<(PeerId, UserId), getrandom::Error> {
	let peer = PeerId(getrandom::u64()?);
	Ok((peer, UserId(peer.0)))
}
