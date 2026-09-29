use super::utility_types::{HistoryDeltaRow, HistoryPanelState, HistoryProgressRow, HistoryRow};
use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::sync::identity::{anonymous_name, user_color};
use crate::messages::prelude::*;
use document_format::GddV1;
use document_graph_storage::{Delta, HistoryMetadata, PeerId, RegistryDelta, Rev, Session, UserId};
use peer_transport::RemotePeer;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[derive(ExtractField)]
pub struct HistoryMessageContext<'a> {
	pub documents: &'a HashMap<DocumentId, DocumentMessageHandler>,
	pub active_document_id: Option<DocumentId>,
	/// Whether the History panel is showing; nothing is computed while it is not.
	pub panel_open: bool,
}

/// Feeds the History panel: the active document's history as interactions along the head's line, sent
/// again whenever it moved. Read-only for now; the actions come with the graph view.
#[derive(Debug, Default, ExtractField)]
pub struct HistoryMessageHandler {
	/// Whether the per-frame tick is subscribed.
	ticking: bool,
	/// Interactions shown with their deltas, by closing rev.
	expanded: HashSet<Rev>,
	/// How many interactions of the head's line to send; grows on request.
	limit: usize,
	/// What the last send described, so a frame with no movement sends nothing.
	last_sent: Option<u64>,
}

/// Interactions sent at first, and added per request for older ones.
const PAGE: usize = 100;

#[message_handler_data]
impl MessageHandler<HistoryMessage, HistoryMessageContext<'_>> for HistoryMessageHandler {
	fn process_message(&mut self, message: HistoryMessage, responses: &mut VecDeque<Message>, context: HistoryMessageContext) {
		match message {
			HistoryMessage::Refresh => {
				if !self.ticking {
					self.ticking = true;
					responses.add(BroadcastMessage::SubscribeEvent {
						on: EventMessage::AnimationFrame,
						send: Box::new(HistoryMessage::Tick.into()),
					});
				}
				self.send(&context, responses, true);
			}
			HistoryMessage::Tick => {
				if context.panel_open {
					self.send(&context, responses, false);
				}
			}
			HistoryMessage::Expand { id, expanded } => {
				if let Some(rev) = id.parse::<u128>().ok().and_then(Rev::new) {
					match expanded {
						true => self.expanded.insert(rev),
						false => self.expanded.remove(&rev),
					};
				}
				self.send(&context, responses, true);
			}
			HistoryMessage::LoadMore => {
				self.limit = self.limit.max(PAGE) + PAGE;
				self.send(&context, responses, true);
			}
		}
	}

	advertise_actions!(HistoryMessageDiscriminant;);
}

impl HistoryMessageHandler {
	fn send(&mut self, context: &HistoryMessageContext, responses: &mut VecDeque<Message>, force: bool) {
		let gdd = context.active_document_id.and_then(|id| context.documents.get(&id)).and_then(|document| document.storage());
		let Some(gdd) = gdd else {
			if force {
				self.last_sent = None;
				responses.add(FrontendMessage::UpdateHistoryPanel { state: HistoryPanelState::default() });
			}
			return;
		};
		let limit = self.limit.max(PAGE);
		let fingerprint = fingerprint(context.active_document_id, gdd, limit, &self.expanded);
		if !force && self.last_sent == Some(fingerprint) {
			return;
		}
		self.last_sent = Some(fingerprint);
		let state = panel_state(gdd, limit, &self.expanded);
		responses.add(FrontendMessage::UpdateHistoryPanel { state });
	}
}

/// Everything the panel's content depends on, hashed: cheap to take every frame.
fn fingerprint(document_id: Option<DocumentId>, gdd: &GddV1, limit: usize, expanded: &HashSet<Rev>) -> u64 {
	let session = gdd.session();
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	document_id.hash(&mut hasher);
	session.history_len().hash(&mut hasher);
	session.head_rev().hash(&mut hasher);
	session.redo_stack().hash(&mut hasher);
	session.hot_log().len().hash(&mut hasher);
	for record in &gdd.metadata().users {
		record.user.hash(&mut hasher);
		if let Some(name) = gdd.metadata().user_name(record.user) {
			name.hash(&mut hasher);
		}
	}
	for remote in gdd.peers() {
		remote.user.hash(&mut hasher);
		remote.name.hash(&mut hasher);
	}
	limit.hash(&mut hasher);
	expanded.len().hash(&mut hasher);
	hasher.finish()
}

fn panel_state(gdd: &GddV1, limit: usize, expanded: &HashSet<Rev>) -> HistoryPanelState {
	let session = gdd.session();
	let people = People {
		session,
		metadata: gdd.metadata(),
		peers: gdd.peers(),
	};

	// Where branches leave the lines shown: every delta's children, minus the lines themselves.
	let mut children: HashMap<Rev, Vec<Rev>> = HashMap::new();
	for delta in session.history() {
		for parent in delta.all_parents() {
			children.entry(parent).or_default().push(delta.id);
		}
	}

	let head = session.head_rev();
	// Taken back and not yet redone: from the redo stack's top down to the head, shown above it.
	let undone = match session.redo_stack().last() {
		Some(&checkpoint) => line(session, Some(checkpoint), head, usize::MAX).0,
		None => Vec::new(),
	};
	let (kept, more) = line(session, head, None, limit);
	let on_line: HashSet<Rev> = undone.iter().chain(&kept).map(|delta| delta.id).collect();

	let mut rows = Vec::new();
	for group in interactions(&undone) {
		rows.push(row(&group, true, false, &children, &on_line, &people, expanded));
	}
	for (index, group) in interactions(&kept).into_iter().enumerate() {
		rows.push(row(&group, false, index == 0, &children, &on_line, &people, expanded));
	}

	// The hot log: what is under way, by the person doing it.
	let mut ops_by_peer: Vec<(PeerId, usize)> = Vec::new();
	for hot_op in session.hot_log() {
		match ops_by_peer.iter_mut().find(|(peer, _)| *peer == hot_op.timestamp.peer) {
			Some((_, count)) => *count += 1,
			None => ops_by_peer.push((hot_op.timestamp.peer, 1)),
		}
	}
	let progress = ops_by_peer
		.into_iter()
		.map(|(peer, ops)| {
			let person = people.person(peer);
			HistoryProgressRow {
				author: person.name,
				anonymous: person.anonymous,
				mine: person.mine,
				color: person.color,
				ops,
			}
		})
		.collect();

	HistoryPanelState { rows, progress, more }
}

/// The deltas from `from` back along first parents to `until` exclusive, or the root, newest first.
/// Stops after `limit` interaction ends and says so.
fn line(session: &Session, from: Option<Rev>, until: Option<Rev>, limit: usize) -> (Vec<Delta>, bool) {
	let mut deltas = Vec::new();
	let mut ends = 0;
	let mut current = from;
	while let Some(rev) = current
		&& Some(rev) != until
	{
		let Some(delta) = session.delta(rev) else { break };
		if delta.is_interaction_end() {
			if ends == limit {
				return (deltas, true);
			}
			ends += 1;
		}
		deltas.push(delta.clone());
		current = delta.parent;
	}
	(deltas, false)
}

/// Group a line, newest first, into interactions: each starts at an interaction end and runs back to the
/// delta after the previous end. A line whose newest delta is not an end, work retired mid-interaction,
/// opens a group of its own.
fn interactions(deltas: &[Delta]) -> Vec<Vec<&Delta>> {
	let mut groups: Vec<Vec<&Delta>> = Vec::new();
	for delta in deltas {
		match groups.last_mut() {
			Some(group) if !delta.is_interaction_end() => group.push(delta),
			_ => groups.push(vec![delta]),
		}
	}
	groups
}

#[allow(clippy::too_many_arguments)]
fn row(group: &[&Delta], undone: bool, head: bool, children: &HashMap<Rev, Vec<Rev>>, on_line: &HashSet<Rev>, people: &People, expanded: &HashSet<Rev>) -> HistoryRow {
	let closing = group[0];
	let person = people.person(closing.author);
	let branches = group
		.iter()
		.flat_map(|delta| children.get(&delta.id).into_iter().flatten())
		.filter(|child| !on_line.contains(child))
		.count();
	let is_expanded = expanded.contains(&closing.id);
	let details = match is_expanded {
		true => group
			.iter()
			.map(|delta| {
				let author = people.person(delta.author);
				HistoryDeltaRow {
					id: delta.id.to_string(),
					label: describe(&delta.kind),
					author: author.name,
					color: author.color,
					time: delta.retired_at().map(|ms| ms as f64),
				}
			})
			.collect(),
		false => Vec::new(),
	};
	HistoryRow {
		id: closing.id.to_string(),
		label: summarize(group),
		author: person.name,
		anonymous: person.anonymous,
		mine: person.mine,
		color: person.color,
		time: group.iter().find_map(|delta| delta.retired_at()).map(|ms| ms as f64),
		deltas: group.len(),
		branches,
		undone,
		head,
		expanded: is_expanded,
		details,
	}
}

/// One line for an interaction: the kinds of change it made, the most frequent first.
fn summarize(group: &[&Delta]) -> String {
	let mut counts: Vec<(String, usize)> = Vec::new();
	for delta in group {
		// A registration says nothing about the document; it labels an interaction only when it is all there is.
		if matches!(delta.kind, RegistryDelta::RegisterPeer { .. } | RegistryDelta::EndTransaction) && group.len() > 1 {
			continue;
		}
		let label = describe(&delta.kind);
		match counts.iter_mut().find(|(known, _)| *known == label) {
			Some((_, count)) => *count += 1,
			None => counts.push((label, 1)),
		}
	}
	counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
	let mut parts: Vec<String> = counts
		.iter()
		.take(2)
		.map(|(label, count)| match count {
			1 => label.clone(),
			count => format!("{label} ×{count}"),
		})
		.collect();
	if counts.len() > 2 {
		parts.push(format!("{} more", counts.len() - 2));
	}
	parts.join(", ")
}

fn describe(kind: &RegistryDelta) -> String {
	match kind {
		RegistryDelta::AddNode { .. } => "Added a node".into(),
		RegistryDelta::RemoveNode { .. } => "Removed a node".into(),
		RegistryDelta::ChangeNodeInput { .. } => "Changed an input".into(),
		RegistryDelta::SetNodeInputs { .. } => "Set a node's inputs".into(),
		RegistryDelta::SetNodeImplementation { .. } => "Changed what a node does".into(),
		RegistryDelta::ChangeNodeAttribute { delta, .. } => format!("Set node {}", delta.key),
		RegistryDelta::ChangeNodeInputAttribute { delta, .. } => format!("Set input {}", delta.key),
		RegistryDelta::SetNetworkExport { .. } => "Changed an export".into(),
		RegistryDelta::ChangeNetworkAttribute { delta, .. } => format!("Set network {}", delta.key),
		RegistryDelta::AddNetwork { .. } => "Added a network".into(),
		RegistryDelta::RemoveNetwork { .. } => "Removed a network".into(),
		RegistryDelta::AddResource { .. } => "Added a resource".into(),
		RegistryDelta::SetResourceHash { .. } => "Updated a resource".into(),
		RegistryDelta::RemoveResource { .. } => "Removed a resource".into(),
		RegistryDelta::AddSource { .. } | RegistryDelta::RemoveSource { .. } => "Changed where a resource comes from".into(),
		RegistryDelta::RegisterPeer { .. } => "Joined the document".into(),
		RegistryDelta::ChangeDocumentAttribute { delta } => format!("Set document {}", delta.key),
		RegistryDelta::Merge { .. } => "Joined two lines of work".into(),
		RegistryDelta::EndTransaction => "Closed a step".into(),
		RegistryDelta::Other(_) => "Changed something".into(),
	}
}

/// How authors are named and coloured: the document's record first, then what the room announced, then
/// the stand-in, all by user id so a person is one person across their peers.
struct People<'a> {
	session: &'a Session,
	metadata: &'a HistoryMetadata,
	peers: Vec<RemotePeer>,
}

struct Person {
	name: String,
	anonymous: bool,
	mine: bool,
	color: String,
}

impl People<'_> {
	fn person(&self, peer: PeerId) -> Person {
		// A peer that never registered, from a copy older than registrations, is its own user by convention.
		let user = self.session.user_of(peer).unwrap_or(UserId(peer.0));
		let recorded = self.metadata.user_name(user).map(str::to_string);
		let announced = || self.peers.iter().find(|remote| remote.user == user && !remote.name.is_empty()).map(|remote| remote.name.clone());
		let (name, anonymous) = match recorded.or_else(announced) {
			Some(name) => (name, false),
			None => (anonymous_name(user), true),
		};
		Person {
			name,
			anonymous,
			mine: self.session.is_mine(peer),
			color: user_color(user),
		}
	}
}
