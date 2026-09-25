use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::document::utility_types::network_interface::TransactionStatus;
use crate::messages::prelude::*;
use crate::messages::resource_storage::ResourcesHandle;
use document_graph_storage::UserId;
use graph_craft::application_io::resource::LoadResource;
use peer_transport::{DEFAULT_SIGNALING_SERVER, Event, Incoming, Room, SessionToken, SyncTarget};
use std::collections::HashSet;

#[derive(ExtractField)]
pub struct SyncMessageContext<'a> {
	pub documents: &'a mut HashMap<DocumentId, DocumentMessageHandler>,
	pub active_document_id: Option<DocumentId>,
	pub resource_storage: &'a ResourceStorageMessageHandler,
}

/// Drives every document's collaborative session: attaches transports, polls them once per frame and as
/// packets arrive, and turns remote changes into interface rebuilds.
#[derive(Debug, Default, ExtractField)]
pub struct SyncMessageHandler {
	pending_join: Option<SessionToken>,
	polling: bool,
	/// Each connected document's room inbox, tagged with the connection it belongs to. One wake future is in
	/// flight per entry: it resolves to a `Wake`, which polls and arms the next one.
	incoming: HashMap<DocumentId, (u32, Incoming)>,
	connections: u32,
	/// Documents whose registry changed remotely since their interface was last rebuilt.
	dirty: HashSet<DocumentId>,
	/// Documents that just took the host's state on: once their interface follows and the graph has run, the
	/// viewport is fitted to the document, so a guest sees what it joined rather than an empty canvas.
	fit_after_sync: HashSet<DocumentId>,
	/// When each document connected to its room still undecided, so the host role is taken once no host has
	/// greeted it for the grace period.
	undecided_since: HashMap<DocumentId, f64>,
	blocked_reason: HashMap<DocumentId, String>,
}

#[message_handler_data]
impl MessageHandler<SyncMessage, SyncMessageContext<'_>> for SyncMessageHandler {
	fn process_message(&mut self, message: SyncMessage, responses: &mut VecDeque<Message>, context: SyncMessageContext) {
		let SyncMessageContext {
			documents,
			active_document_id,
			resource_storage,
		} = context;

		match message {
			SyncMessage::Share => {
				let Some(document) = active_document_id.and_then(|id| documents.get_mut(&id)) else { return };
				let Some(gdd) = document.storage_mut() else {
					log::warn!("Cannot share a document before its working copy is mounted");
					return;
				};
				if gdd.role().is_some() {
					return;
				}

				let document_id = active_document_id.expect("checked above");
				let Some(token) = self.connect_document(document_id, gdd, responses) else { return };
				responses.add(FrontendMessage::TriggerSessionLinkCopy { token: token.to_string() });
				responses.add(DialogMessage::DisplayDialogError {
					title: "Live session started".into(),
					description: format!("A link to join was copied to the clipboard.\n\nSession {token}"),
				});
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				self.start_polling(responses);
			}
			SyncMessage::Join { token } => {
				let token = match token.parse::<SessionToken>() {
					Ok(token) => token,
					Err(error) => {
						log::warn!("Ignoring session link: {error}");
						return;
					}
				};
				self.pending_join = Some(token);
				responses.add(PortfolioMessage::NewDocumentWithName { name: "Shared session".into() });
			}
			SyncMessage::StorageMounted { document_id } => {
				let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) else {
					return;
				};
				if let Some(token) = self.pending_join.take() {
					let (room, driver) = Room::connect(&token.signaling_url(DEFAULT_SIGNALING_SERVER));
					let incoming = room.incoming();
					let user = UserId(gdd.session().peer().0);
					gdd.join(room, user);

					responses.add(driver_future(document_id, driver));
					self.attach(document_id, incoming, responses);
					responses.add(PortfolioMessage::UpdateOpenDocumentsList);
					self.start_polling(responses);
				} else if gdd.is_shared() && gdd.role().is_none() {
					// The document was in its room when it was last persisted: a reload rejoins on its own.
					self.connect_document(document_id, gdd, responses);
				}
			}
			SyncMessage::Leave => {
				let Some(document_id) = active_document_id else { return };
				let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) else {
					return;
				};
				gdd.leave();
				self.incoming.remove(&document_id);
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			SyncMessage::Disconnected { document_id } => {
				if let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) {
					gdd.leave();
				}
				self.incoming.remove(&document_id);
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			SyncMessage::Poll | SyncMessage::Wake { .. } => {
				// A wake names the connection it was armed for; one from a connection since left is nothing to act on.
				let woken = match message {
					SyncMessage::Wake { document_id, generation } => {
						if self.incoming.get(&document_id).is_none_or(|(current, _)| *current != generation) {
							return;
						}
						Some(document_id)
					}
					_ => None,
				};
				let resources = resource_storage.resources_mut();
				for (&document_id, document) in documents.iter_mut() {
					// Retirement follows the working copy's policy on every document, in a session or not: closed
					// transactions retire once enough have waited long enough, never on a gesture.
					let idle = document.network_interface.transaction_status() == TransactionStatus::Finished;
					if let Some(gdd) = document.storage_mut()
						&& let Err(error) = gdd.retire_due(now_ms(), idle)
					{
						log::error!("Retirement failed: {error}");
					}
					if document.storage().is_none_or(|gdd| gdd.role().is_none()) {
						continue;
					}
					// A document connected without knowing who hosts takes the role itself once no host has greeted
					// it for the grace period.
					if self.undecided_since.get(&document_id).is_some_and(|since| now_ms() - since >= ROLE_GRACE_MS)
						&& let Some(gdd) = document.storage_mut()
						&& gdd.role() == Some(peer_transport::Role::Undecided)
						&& gdd.decide_role().is_some()
					{
						self.undecided_since.remove(&document_id);
						responses.add(PortfolioMessage::UpdateOpenDocumentsList);
					}
					// Each movement reaches peers as it happens: what the interface recorded since the last
					// frame is staged, and so broadcast, ahead of this frame's poll.
					document.stage_pending_edits(&resources);
					let Some(gdd) = document.storage_mut() else { continue };

					let events = gdd.poll_peers();
					// Taken every poll rather than on an event: a full sync a hello triggers can replace the
					// registry with no change event to announce it.
					let changes = gdd.take_remote_changes();
					if !changes.is_empty() {
						document.pending_remote.extend(changes);
						self.dirty.insert(document_id);
					}
					for event in events {
						match event {
							Event::Synced => {
								log::info!("Join handshake: synced, applying the host's state");
								self.dirty.insert(document_id);
								self.fit_after_sync.insert(document_id);
							}
							Event::Changed => {
								self.dirty.insert(document_id);
							}
							Event::ResourceRequested { from, hash } => {
								log::debug!("Peer asked for resource {hash}");
								responses.add(load_resource_future(document_id, from, hash, resources.clone()));
							}
							// The replica already put the bytes in the byte store; only the decoded form is missing.
							Event::ResourceReceived { hash, bytes } => {
								log::debug!("Received resource {hash} ({} bytes)", bytes.len());
								document.cache_declaration_bytes(hash, &bytes);
								self.dirty.insert(document_id);
							}
							Event::PeerJoined { .. } | Event::PeerLeft { .. } => responses.add(PortfolioMessage::UpdateOpenDocumentsList),
						}
					}
				}

				// Apply only once every referenced resource is in the app cache, and never underneath an
				// open transaction, whose tool state names nodes a peer may have removed.
				for document_id in self.dirty.clone() {
					let Some(document) = documents.get_mut(&document_id) else {
						self.dirty.remove(&document_id);
						continue;
					};
					let Some(gdd) = document.storage() else { continue };
					let missing = SyncTarget::missing_resources(gdd).len();
					let transaction = document.network_interface.transaction_status();
					if missing > 0 || transaction != TransactionStatus::Finished {
						let reason = format!("missing {missing} transaction {transaction:?}");
						if self.blocked_reason.get(&document_id) != Some(&reason) {
							log::debug!("Applying remote changes to {document_id:?} waits: {reason}");
							self.blocked_reason.insert(document_id, reason);
						}
						continue;
					}
					self.blocked_reason.remove(&document_id);

					// Stays dirty while a declaration the changes need is still on its way.
					if document.apply_remote_changes(responses) {
						self.dirty.remove(&document_id);
						if self.fit_after_sync.remove(&document_id) && Some(document_id) == active_document_id {
							log::info!("Join handshake: the host's state is applied, fitting the viewport after the graph runs");
							// The bounds come from the render, so the fit waits for the graph to run on the new document.
							responses.add(DeferMessage::AfterGraphRun {
								messages: vec![DocumentMessage::ZoomCanvasToFitAll.into()],
							});
						}
					}
					// The registry names resources this peer may still have to fetch or resolve.
					responses.add(PortfolioMessage::ResolveDocumentResources { document_id });
				}

				// The inbox was drained above; the next arrival wakes the next poll.
				if let Some(document_id) = woken {
					self.arm(document_id, responses);
				}
			}
			SyncMessage::ResourceLoaded { document_id, to, hash, bytes } => {
				log::debug!("Sending resource {hash} ({} bytes)", bytes.len());
				let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) else {
					return;
				};
				if let Err(error) = gdd.send_resource(to, hash, bytes) {
					log::warn!("Failed to send resource {hash} to a peer: {error}");
				}
			}
		}
	}

	advertise_actions!(SyncMessageDiscriminant; Share, Leave);
}

impl SyncMessageHandler {
	/// Connect `document_id`'s working copy to the room every copy of the document shares, and hand the
	/// frontend the link. Returns the token, `None` when the connection could not be set up.
	fn connect_document(&mut self, document_id: DocumentId, gdd: &mut document_format::GddV1, responses: &mut VecDeque<Message>) -> Option<SessionToken> {
		let token = SessionToken::for_document(gdd.manifest().document_id);
		let (room, driver) = Room::connect(&token.signaling_url(DEFAULT_SIGNALING_SERVER));
		let incoming = room.incoming();
		let user = UserId(gdd.session().peer().0);
		if let Err(error) = gdd.connect(room, user) {
			log::error!("Connecting to the session failed: {error}");
			return None;
		}
		self.undecided_since.insert(document_id, now_ms());
		responses.add(driver_future(document_id, driver));
		self.attach(document_id, incoming, responses);
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		self.start_polling(responses);
		Some(token)
	}

	/// Poll `document_id`'s room as packets arrive, not only per frame: a hidden browser tab gets about one
	/// frame a second, which made a host answer each step of a join a second late. The wake rides on the
	/// editor's future plumbing, so it works the same on the desktop as in the browser.
	fn attach(&mut self, document_id: DocumentId, incoming: Incoming, responses: &mut VecDeque<Message>) {
		self.connections += 1;
		self.incoming.insert(document_id, (self.connections, incoming));
		self.arm(document_id, responses);
	}

	/// Put one wake future in flight for `document_id`'s current connection.
	fn arm(&mut self, document_id: DocumentId, responses: &mut VecDeque<Message>) {
		let Some((generation, incoming)) = self.incoming.get(&document_id).cloned() else { return };
		let future = async move {
			match incoming.wait().await {
				true => Message::Portfolio(PortfolioMessage::Sync(SyncMessage::Wake { document_id, generation })),
				// The room's loop ended; its driver reports the disconnection.
				false => Message::NoOp,
			}
		};
		responses.add(future);
	}

	fn start_polling(&mut self, responses: &mut VecDeque<Message>) {
		if self.polling {
			return;
		}
		self.polling = true;
		responses.add(BroadcastMessage::SubscribeEvent {
			on: EventMessage::AnimationFrame,
			send: Box::new(SyncMessage::Poll.into()),
		});
	}
}

fn driver_future(document_id: DocumentId, driver: peer_transport::MessageLoopFuture) -> Message {
	let future = async move {
		if let Err(error) = driver.await {
			log::warn!("Session transport for {document_id:?} ended: {error}");
		}
		Message::Portfolio(PortfolioMessage::Sync(SyncMessage::Disconnected { document_id }))
	};
	future.into()
}

fn load_resource_future(document_id: DocumentId, to: peer_transport::TransportPeerId, hash: graph_craft::application_io::resource::ResourceHash, resources: ResourcesHandle) -> Message {
	let future = async move {
		match resources.load(hash).await {
			Some(resource) => Message::Portfolio(PortfolioMessage::Sync(SyncMessage::ResourceLoaded {
				document_id,
				to,
				hash,
				bytes: resource.as_ref().to_vec(),
			})),
			None => {
				log::warn!("A peer asked for resource {hash} which is not in the byte store");
				Message::NoOp
			}
		}
	};
	future.into()
}

/// How long a peer that connected to an empty-looking room waits for a host's hello before it takes the
/// role itself.
const ROLE_GRACE_MS: f64 = 1_500.;

/// A monotonic-enough millisecond clock for the retirement policy, which only ever compares differences.
fn now_ms() -> f64 {
	#[cfg(target_arch = "wasm32")]
	{
		js_sys::Date::now()
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map(|elapsed| elapsed.as_secs_f64() * 1000.)
			.unwrap_or(0.)
	}
}
