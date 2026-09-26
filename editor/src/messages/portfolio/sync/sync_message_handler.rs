use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::document::overlays::utility_types::{OverlayProvider, Pivot};
use crate::messages::portfolio::document::utility_types::network_interface::TransactionStatus;
use crate::messages::portfolio::utility_types::PanelType;
use crate::messages::prelude::*;
use crate::messages::resource_storage::ResourcesHandle;
use crate::messages::viewport::Position;
use document_graph_storage::{PeerId, UserId};
use glam::{DAffine2, DVec2};
use graph_craft::application_io::resource::{LoadResource, ResourceHash};
use peer_transport::{DEFAULT_SIGNALING_SERVER, Event, Incoming, RemotePeer, Role, Room, SessionToken, SyncTarget};
use std::collections::HashSet;

#[derive(ExtractField)]
pub struct SyncMessageContext<'a> {
	pub documents: &'a mut HashMap<DocumentId, DocumentMessageHandler>,
	pub active_document_id: Option<DocumentId>,
	pub resource_storage: &'a ResourceStorageMessageHandler,
	pub preferences: &'a PreferencesMessageHandler,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub viewport: &'a ViewportMessageHandler,
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
	/// Declarations being read from the byte store to be decoded, so a document waiting on them asks once.
	decoding: HashSet<(DocumentId, ResourceHash)>,
	/// The pointer position last sent for each connected document, so one is sent only when it moved.
	last_cursor: HashMap<DocumentId, Option<[f64; 2]>>,
}

#[message_handler_data]
impl MessageHandler<SyncMessage, SyncMessageContext<'_>> for SyncMessageHandler {
	fn process_message(&mut self, message: SyncMessage, responses: &mut VecDeque<Message>, context: SyncMessageContext) {
		let SyncMessageContext {
			documents,
			active_document_id,
			resource_storage,
			preferences,
			ipp,
			viewport,
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
				let Some(token) = self.connect_document(document_id, gdd, preferences, responses) else { return };
				// The panel shows the link; the clipboard gets it too, so sharing stays one step.
				responses.add(FrontendMessage::TriggerSessionLinkCopy { token: token.to_string() });
				responses.add(WorkspaceMessage::FocusPanel { panel_type: PanelType::Session });
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
					gdd.join(room, UserId(preferences.user_id));
					announce_name(gdd, &preferences.user_name);

					responses.add(driver_future(document_id, driver));
					self.attach(document_id, incoming, responses);
					refresh_session_views(responses);
					self.start_polling(responses);
				} else if gdd.is_shared() && gdd.role().is_none() {
					// The document was in its room when it was last persisted: a reload rejoins on its own.
					self.connect_document(document_id, gdd, preferences, responses);
				}
			}
			SyncMessage::Leave => {
				let Some(document_id) = active_document_id else { return };
				let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) else {
					return;
				};
				gdd.leave();
				self.incoming.remove(&document_id);
				self.last_cursor.remove(&document_id);
				refresh_session_views(responses);
			}
			SyncMessage::Disconnected { document_id } => {
				if let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) {
					gdd.leave();
				}
				self.incoming.remove(&document_id);
				self.last_cursor.remove(&document_id);
				refresh_session_views(responses);
			}
			SyncMessage::RefreshPanel => {
				let layout = session_panel_layout(documents, active_document_id, &preferences.user_name);
				responses.add(LayoutMessage::SendLayout {
					layout,
					layout_target: LayoutTarget::SessionPanel,
				});
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
					// A document without a host, connected that way or left that way by a host that went before
					// syncing it, takes the role itself once no host has greeted it for the grace period.
					if document.storage().is_some_and(|gdd| gdd.role() == Some(peer_transport::Role::Undecided)) {
						let since = *self.undecided_since.entry(document_id).or_insert_with(now_ms);
						if now_ms() - since >= ROLE_GRACE_MS
							&& let Some(gdd) = document.storage_mut()
							&& gdd.decide_role().is_some()
						{
							self.undecided_since.remove(&document_id);
							refresh_session_views(responses);
						}
					} else {
						self.undecided_since.remove(&document_id);
					}
					// Each movement reaches peers as it happens: what the interface recorded since the last
					// frame is staged, and so broadcast, ahead of this frame's poll.
					document.stage_pending_edits(&resources);
					// Presence rides outside the causal broadcast: the name whenever it changed, and the pointer
					// over the active document once per frame when it moved, `None` for every other document.
					let cursor = (Some(document_id) == active_document_id).then(|| cursor_in_document(document, ipp, viewport)).flatten();
					let Some(gdd) = document.storage_mut() else { continue };
					announce_name(gdd, &preferences.user_name);
					if self.last_cursor.get(&document_id) != Some(&cursor) {
						match gdd.send_cursor(cursor) {
							Ok(()) => {
								self.last_cursor.insert(document_id, cursor);
							}
							Err(error) => log::warn!("Sending the pointer position failed: {error}"),
						}
					}

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
								refresh_session_views(responses);
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
							Event::RoleChanged { role } => {
								log::info!("Session role is now {role:?}");
								refresh_session_views(responses);
							}
							Event::PeerJoined { .. } | Event::PeerLeft { .. } | Event::ProfileChanged { .. } => refresh_session_views(responses),
							Event::CursorMoved { .. } => {
								if Some(document_id) == active_document_id {
									responses.add(OverlaysMessage::Draw);
								}
							}
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
					// Every resource is on hand, but a declaration held since an earlier session was never received from
					// a peer and so never decoded: read it from the byte store, and apply once it is cached.
					let undecoded = document.undecoded_declaration_hashes();
					if !undecoded.is_empty() {
						let reason = format!("decoding {} held declarations", undecoded.len());
						if self.blocked_reason.get(&document_id) != Some(&reason) {
							log::debug!("Applying remote changes to {document_id:?} waits: {reason}");
							self.blocked_reason.insert(document_id, reason);
						}
						for hash in undecoded {
							if self.decoding.insert((document_id, hash)) {
								responses.add(decode_declaration_future(document_id, hash, resources.clone()));
							}
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
			SyncMessage::DrawPresence { context: mut overlay_context } => {
				let Some(document) = active_document_id.and_then(|id| documents.get(&id)) else { return };
				let Some(gdd) = document.storage().filter(|gdd| gdd.role().is_some()) else { return };
				let to_viewport = document.metadata().document_to_viewport;
				for remote in gdd.peers() {
					let Some([x, y]) = remote.cursor else { continue };
					let position = to_viewport.transform_point2(DVec2::new(x, y));
					let color = peer_color(remote.peer);
					let arrow = CURSOR_ARROW.map(|[dx, dy]| position + DVec2::new(dx, dy));
					overlay_context.polygon(&arrow, Some("#ffffff"), Some(&color));
					let label = DAffine2::from_translation(position + DVec2::new(14., 18.));
					overlay_context.text(&display_name(&remote), "#ffffff", Some(&color), label, 3., [Pivot::Start, Pivot::Start]);
				}
			}
			SyncMessage::DeclarationLoaded { document_id, hash, bytes } => {
				let Some(bytes) = bytes else {
					// Stays in `decoding` so it is not asked for again every frame; the document waits on it.
					log::warn!("Declaration {hash} is recorded as held but the byte store has no bytes for it");
					return;
				};
				self.decoding.remove(&(document_id, hash));
				let Some(document) = documents.get_mut(&document_id) else { return };
				log::debug!("Decoded held declaration {hash} ({} bytes)", bytes.len());
				document.cache_declaration_bytes(hash, &bytes);
				self.dirty.insert(document_id);
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
	fn connect_document(&mut self, document_id: DocumentId, gdd: &mut document_format::GddV1, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) -> Option<SessionToken> {
		let token = SessionToken::for_document(gdd.manifest().document_id);
		let (room, driver) = Room::connect(&token.signaling_url(DEFAULT_SIGNALING_SERVER));
		let incoming = room.incoming();
		if let Err(error) = gdd.connect(room, UserId(preferences.user_id)) {
			log::error!("Connecting to the session failed: {error}");
			return None;
		}
		announce_name(gdd, &preferences.user_name);
		self.undecided_since.insert(document_id, now_ms());
		responses.add(driver_future(document_id, driver));
		self.attach(document_id, incoming, responses);
		refresh_session_views(responses);
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

/// Read a declaration the byte store already holds, so it can be decoded for a remote change that names it.
fn decode_declaration_future(document_id: DocumentId, hash: ResourceHash, resources: ResourcesHandle) -> Message {
	let future = async move {
		let bytes = resources.load(hash).await.map(|resource| resource.as_ref().to_vec());
		Message::Portfolio(PortfolioMessage::Sync(SyncMessage::DeclarationLoaded { document_id, hash, bytes }))
	};
	future.into()
}

fn load_resource_future(document_id: DocumentId, to: peer_transport::TransportPeerId, hash: ResourceHash, resources: ResourcesHandle) -> Message {
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

/// The session state changed: the document tabs show it as a circle and the Session panel in full.
fn refresh_session_views(responses: &mut VecDeque<Message>) {
	responses.add(PortfolioMessage::UpdateOpenDocumentsList);
	responses.add(SyncMessage::RefreshPanel);
}

/// The Session panel for the active document: its state in words, the join link, the peers, and the actions
/// that apply in that state.
fn session_panel_layout(documents: &HashMap<DocumentId, DocumentMessageHandler>, active_document_id: Option<DocumentId>, user_name: &str) -> Layout {
	let heading = |text: &str| LayoutGroup::row(vec![TextLabel::new(text).bold(true).widget_instance()]);
	let note = |text: &str| LayoutGroup::row(vec![TextLabel::new(text).multiline(true).widget_instance()]);
	// The name is a preference, so it is the same in every session and asked for once.
	let name_row = || {
		LayoutGroup::row(vec![
			TextLabel::new("Your name").table_align(true).min_width(80).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(user_name)
				.placeholder("Anonymous")
				.on_update(|input: &TextInput| PreferencesMessage::UserName { name: input.value.clone() }.into())
				.widget_instance(),
		])
	};

	let Some(document) = active_document_id.and_then(|id| documents.get(&id)) else {
		return Layout(vec![note("Open a document to share it.")]);
	};
	let Some(gdd) = document.storage() else {
		return Layout(vec![note("The document's working copy is still being mounted.")]);
	};

	let Some(role) = gdd.role() else {
		if gdd.is_shared() {
			return Layout(vec![
				name_row(),
				heading("Disconnected"),
				note("The document is shared but not in its room right now. It reconnects when it is reopened."),
				LayoutGroup::row(vec![
					TextButton::new("Reconnect").icon("Link").emphasized(true).on_commit(|_| SyncMessage::Share.into()).widget_instance(),
					TextButton::new("Stop Sharing").on_commit(|_| SyncMessage::Leave.into()).widget_instance(),
				]),
			]);
		}
		return Layout(vec![
			name_row(),
			heading("Not shared"),
			note("Share this document to edit it live with others. Everyone who opens the link works on the same document."),
			LayoutGroup::row(vec![
				TextButton::new("Share Live Session")
					.icon("Link")
					.emphasized(true)
					.on_commit(|_| SyncMessage::Share.into())
					.widget_instance(),
			]),
		]);
	};

	let state = match role {
		Role::Host => "Hosting",
		Role::Guest if gdd.is_synced() => "Guest",
		Role::Guest => "Joining, waiting for the host's state",
		Role::Undecided => "Connected, waiting for a host",
	};
	let token = SessionToken::for_document(gdd.manifest().document_id).to_string();
	let copy_token = token.clone();
	let mut peers = gdd.peers();
	peers.sort_by_key(|remote| remote.peer);

	let mut groups = vec![
		name_row(),
		heading(state),
		LayoutGroup::row(vec![
			TextLabel::new(token)
				.monospace(true)
				.selectable(true)
				.tooltip_label("Session")
				.tooltip_description("Opening the editor with this session in the link joins the room.")
				.widget_instance(),
		]),
		LayoutGroup::row(vec![
			TextButton::new("Copy Link")
				.icon("Copy")
				.emphasized(true)
				.on_commit(move |_| FrontendMessage::TriggerSessionLinkCopy { token: copy_token.clone() }.into())
				.widget_instance(),
			TextButton::new("Disconnect").on_commit(|_| SyncMessage::Leave.into()).widget_instance(),
		]),
		heading(&format!("Peers ({})", peers.len() + 1)),
		peer_row(&if user_name.is_empty() { "You".to_string() } else { format!("{user_name} (you)") }, role),
	];
	groups.extend(peers.iter().map(|remote| peer_row(&display_name(remote), remote.role)));
	Layout(groups)
}

fn peer_row(name: &str, role: Role) -> LayoutGroup {
	let role = match role {
		Role::Host => "Host",
		Role::Guest => "Guest",
		Role::Undecided => "Deciding",
	};
	LayoutGroup::row(vec![
		TextLabel::new(name).min_width(120).widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		TextLabel::new(role).disabled(true).widget_instance(),
	])
}

/// Draws the other peers' cursors; registered on every document with the tools, and a no-op outside a session.
pub const PRESENCE_OVERLAY_PROVIDER: OverlayProvider = |context| SyncMessage::DrawPresence { context }.into();

/// A pointer arrow in logical pixels, tip at the origin.
const CURSOR_ARROW: [[f64; 2]; 7] = [[0., 0.], [0., 15.], [4., 11.5], [7., 17.5], [9.5, 16.5], [6.5, 10.5], [11., 10.5]];

/// Tell the room this peer's display name; a no-op when it has not changed.
fn announce_name(gdd: &mut document_format::GddV1, name: &str) {
	if let Err(error) = gdd.set_name(name) {
		log::warn!("Announcing the display name failed: {error}");
	}
}

/// The pointer in document space while it is over the viewport, rounded so jitter below a hundredth of a
/// unit sends nothing.
fn cursor_in_document(document: &DocumentMessageHandler, ipp: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) -> Option<[f64; 2]> {
	let mouse = ipp.mouse.position;
	let size = viewport.size();
	if mouse.x < 0. || mouse.y < 0. || mouse.x > size.x() || mouse.y > size.y() {
		return None;
	}
	let position = document.metadata().document_to_viewport.inverse().transform_point2(mouse);
	Some([(position.x * 100.).round() / 100., (position.y * 100.).round() / 100.])
}

fn display_name(remote: &RemotePeer) -> String {
	match remote.name.is_empty() {
		true => format!("Peer {:06x}", remote.peer.0 & 0xFF_FFFF),
		false => remote.name.clone(),
	}
}

/// A colour for a peer derived from its id, so every peer sees the same one without anything on the wire.
fn peer_color(peer: PeerId) -> String {
	let hue = (peer.0.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) % 360;
	let (h, s, l): (f64, f64, f64) = (hue as f64 / 60., 0.65, 0.5);
	let c = (1. - (2. * l - 1.).abs()) * s;
	let x = c * (1. - (h % 2. - 1.).abs());
	let m = l - c / 2.;
	let (r, g, b) = match h as u32 {
		0 => (c, x, 0.),
		1 => (x, c, 0.),
		2 => (0., c, x),
		3 => (0., x, c),
		4 => (x, 0., c),
		_ => (c, 0., x),
	};
	let channel = |value: f64| ((value + m) * 255.).round() as u8;
	format!("#{:02x}{:02x}{:02x}", channel(r), channel(g), channel(b))
}
