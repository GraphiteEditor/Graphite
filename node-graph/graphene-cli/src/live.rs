//! The `host`, `join` and `watch` subcommands: a memory-backed document in a collaborative session,
//! driven from stdin or shown in a window.

use crate::repl;
use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{PeerId, UserId};
use document_live::{LiveDocument, SessionToken};
use std::error::Error;
use std::hash::{BuildHasher, Hasher, RandomState};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};

pub const DEFAULT_SIGNALING_SERVER: &str = "ws://graphite.kobert.dev:3536";

pub async fn host(signaling: &str, document: Option<&Path>) -> Result<(), Box<dyn Error>> {
	let (peer, user) = identity();
	let token = SessionToken(random_bytes());
	let mut live = match document {
		Some(path) => LiveDocument::host_document(open_archive(path).await?, signaling, token, user, spawn_driver),
		None => LiveDocument::host(signaling, token, peer, user, spawn_driver).await?,
	};
	println!("hosting session {}", live.token());
	repl_loop(&mut live).await;
	live.leave();
	Ok(())
}

pub async fn join(signaling: &str, token: SessionToken) -> Result<(), Box<dyn Error>> {
	let (peer, user) = identity();
	let mut live = LiveDocument::join(signaling, token, peer, user, spawn_driver).await?;
	println!("joining session {token}");
	repl_loop(&mut live).await;
	live.leave();
	Ok(())
}

pub async fn watch(signaling: &str, token: SessionToken) -> Result<(), Box<dyn Error>> {
	let (peer, user) = identity();
	let live = LiveDocument::join(signaling, token, peer, user, spawn_driver).await?;
	crate::watch::run(live).await
}

async fn open_archive(path: &Path) -> Result<GddV1, Box<dyn Error>> {
	let archive = std::fs::read(path).map_err(|error| format!("Failed to read document {}: {error}", path.display()))?;
	let container = AnyContainer::Memory(MemoryBackend::new());
	Ok(GddV1::open_from_archive(&archive, container, GddV1Layout).await?)
}

async fn repl_loop(live: &mut LiveDocument) {
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
				if !repl::run_command(live, line.trim()) {
					break;
				}
			}
		}
	}
}

fn spawn_driver(driver: document_live::MessageLoopFuture) {
	tokio::spawn(driver);
}

/// WebRTC's DTLS goes through rustls, which can't pick between the workspace's `ring` and `aws-lc-rs`.
pub fn install_crypto_provider() {
	let _ = rustls::crypto::ring::default_provider().install_default();
}

fn identity() -> (PeerId, UserId) {
	let peer = PeerId(random_u64());
	(peer, UserId(peer.0))
}

fn random_u64() -> u64 {
	RandomState::new().build_hasher().finish()
}

fn random_bytes() -> [u8; 16] {
	let mut bytes = [0; 16];
	bytes[..8].copy_from_slice(&random_u64().to_le_bytes());
	bytes[8..].copy_from_slice(&random_u64().to_le_bytes());
	bytes
}
