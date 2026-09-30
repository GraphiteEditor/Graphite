use super::utility_types::{HistoryBranch, HistoryDeltaRow, HistoryPanelState, HistoryProgressRow, HistoryRow};
use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::sync::identity::{anonymous_name, user_color};
use crate::messages::portfolio::sync::now_ms;
use crate::messages::prelude::*;
use document_format::GddV1;
use document_graph_storage::{Delta, HistoryMetadata, HotOpId, PeerId, RegistryDelta, Rev, Session, UserId, rev_attr};
use peer_transport::RemotePeer;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[derive(ExtractField)]
pub struct HistoryMessageContext<'a> {
	pub documents: &'a mut HashMap<DocumentId, DocumentMessageHandler>,
	pub active_document_id: Option<DocumentId>,
	/// Whether the History panel is showing; nothing is computed while it is not.
	pub panel_open: bool,
}

/// Feeds the History panel with the active document's history as interactions along a line, sent again
/// whenever it moved, and turns the panel's actions into moves of the document's head.
#[derive(Debug, Default, ExtractField)]
pub struct HistoryMessageHandler {
	/// Whether the per-frame tick is subscribed.
	ticking: bool,
	/// Interactions shown with their deltas, by closing rev.
	expanded: HashSet<Rev>,
	/// How many interactions of the line to send; grows on request.
	limit: usize,
	/// The tip of the branch shown instead of the head's line.
	following: Option<Rev>,
	/// What the last send described, so a frame with no movement sends nothing.
	last_sent: Option<u64>,
}

/// Interactions sent at first, and added per request for older ones.
const PAGE: usize = 100;

#[message_handler_data]
impl MessageHandler<HistoryMessage, HistoryMessageContext<'_>> for HistoryMessageHandler {
	fn process_message(&mut self, message: HistoryMessage, responses: &mut VecDeque<Message>, mut context: HistoryMessageContext) {
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
				if let Some(rev) = parse_rev(&id) {
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
			HistoryMessage::Follow { id } => {
				self.following = id.as_deref().and_then(parse_rev);
				self.send(&context, responses, true);
			}
			HistoryMessage::GoBack { id } => {
				let Some((gdd, rev)) = active_storage(&context).zip(parse_rev(&id)) else { return };
				let session = gdd.session();
				match place(session, rev) {
					// Above the head: forward again over the undone interactions, one redo each.
					Some(Place::Undone(steps)) => (0..steps).for_each(|_| responses.add(DocumentMessage::Redo)),
					// On the head's line: in a session one shared move for everyone; alone, the cursor walks back one
					// interaction at a time so the editor's own undo stays in step.
					Some(Place::Line(0)) => {}
					Some(Place::Line(steps)) => match gdd.role().is_some() {
						true => responses.add(DocumentMessage::HistoryMoveHead { rev }),
						false => (0..steps).for_each(|_| responses.add(DocumentMessage::Undo)),
					},
					None => log::warn!("The History panel asked to go to {rev:?}, which is not on the head's line"),
				}
			}
			HistoryMessage::RemoveStep { id } => {
				if let Some(rev) = parse_rev(&id) {
					responses.add(DocumentMessage::HistoryRemoveStep { rev });
				}
			}
			HistoryMessage::BringBack { id } => {
				let Some((gdd, rev)) = active_storage(&context).zip(parse_rev(&id)) else { return };
				match place(gdd.session(), rev) {
					Some(Place::Undone(steps)) => (0..steps).for_each(|_| responses.add(DocumentMessage::Redo)),
					Some(Place::Line(_)) => {}
					// Off the line: minted again on top of it.
					None => responses.add(DocumentMessage::HistoryRestoreStep { rev }),
				}
			}
			HistoryMessage::Rename { id, label } => {
				let Some((gdd, rev)) = active_storage_mut(&mut context).zip(parse_rev(&id)) else { return };
				if let Err(error) = gdd.record_rev_attribute(rev, rev_attr::LABEL, serde_json::Value::from(label.trim()), now_ms()) {
					log::warn!("Naming the step failed: {error}");
				}
				self.send(&context, responses, true);
			}
			HistoryMessage::Tag { id, tag, on } => {
				let tag = tag.trim();
				if tag.is_empty() {
					return;
				}
				let Some((gdd, rev)) = active_storage_mut(&mut context).zip(parse_rev(&id)) else { return };
				if let Err(error) = gdd.record_rev_attribute(rev, &format!("{}{tag}", rev_attr::TAG_PREFIX), serde_json::Value::Bool(on), now_ms()) {
					log::warn!("Tagging the step failed: {error}");
				}
				self.send(&context, responses, true);
			}
		}
	}

	advertise_actions!(HistoryMessageDiscriminant;);
}

impl HistoryMessageHandler {
	fn send(&mut self, context: &HistoryMessageContext, responses: &mut VecDeque<Message>, force: bool) {
		let Some(gdd) = active_storage(context) else {
			if force {
				self.last_sent = None;
				responses.add(FrontendMessage::UpdateHistoryPanel { state: HistoryPanelState::default() });
			}
			return;
		};
		// A branch whose tip this copy no longer holds is nothing to follow.
		if self.following.is_some_and(|tip| gdd.session().delta(tip).is_none()) {
			self.following = None;
		}
		let limit = self.limit.max(PAGE);
		let fingerprint = fingerprint(context.active_document_id, gdd, limit, &self.expanded, self.following);
		if !force && self.last_sent == Some(fingerprint) {
			return;
		}
		self.last_sent = Some(fingerprint);
		let state = panel_state(gdd, limit, &self.expanded, self.following);
		responses.add(FrontendMessage::UpdateHistoryPanel { state });
	}
}

fn active_storage<'a>(context: &'a HistoryMessageContext) -> Option<&'a GddV1> {
	context.active_document_id.and_then(|id| context.documents.get(&id)).and_then(|document| document.storage())
}

fn active_storage_mut<'a>(context: &'a mut HistoryMessageContext) -> Option<&'a mut GddV1> {
	context.active_document_id.and_then(|id| context.documents.get_mut(&id)).and_then(|document| document.storage_mut())
}

fn parse_rev(id: &str) -> Option<Rev> {
	id.parse::<u128>().ok().and_then(Rev::new)
}

/// Where an interaction sits relative to the head.
enum Place {
	/// On the head's line, this many interactions back; zero is the head itself.
	Line(usize),
	/// Above the head on the undone line, this many redos forward.
	Undone(usize),
}

fn place(session: &Session, rev: Rev) -> Option<Place> {
	let head = session.head_rev();
	let (kept, _) = line(session, head, None, usize::MAX);
	if let Some(steps) = interactions(&kept).iter().position(|group| group[0].id == rev) {
		return Some(Place::Line(steps));
	}
	let checkpoint = *session.redo_stack().last()?;
	let (undone, _) = line(session, Some(checkpoint), head, usize::MAX);
	let groups = interactions(&undone);
	let index = groups.iter().position(|group| group[0].id == rev)?;
	Some(Place::Undone(groups.len() - index))
}

/// Everything the panel's content depends on, hashed: cheap to take every frame.
fn fingerprint(document_id: Option<DocumentId>, gdd: &GddV1, limit: usize, expanded: &HashSet<Rev>, following: Option<Rev>) -> u64 {
	let session = gdd.session();
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	document_id.hash(&mut hasher);
	session.history_len().hash(&mut hasher);
	session.head_rev().hash(&mut hasher);
	session.redo_stack().hash(&mut hasher);
	session.hot_log().len().hash(&mut hasher);
	gdd.role().is_some().hash(&mut hasher);
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
	for record in &gdd.metadata().revs {
		record.rev.hash(&mut hasher);
		for (key, fact) in &record.attributes {
			key.hash(&mut hasher);
			fact.stamp.hash(&mut hasher);
		}
	}
	limit.hash(&mut hasher);
	expanded.len().hash(&mut hasher);
	following.hash(&mut hasher);
	hasher.finish()
}

fn panel_state(gdd: &GddV1, limit: usize, expanded: &HashSet<Rev>, following: Option<Rev>) -> HistoryPanelState {
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
	let (head_line, _) = line(session, head, None, usize::MAX);
	let head_line_revs: HashSet<Rev> = head_line.iter().map(|delta| delta.id).collect();

	// Taken back and not yet redone: from the redo stack's top down to the head, shown above it. Not while
	// following a branch, whose line is shown whole instead.
	let undone = match (following, session.redo_stack().last()) {
		(None, Some(&checkpoint)) => line(session, Some(checkpoint), head, usize::MAX).0,
		_ => Vec::new(),
	};
	let (shown, more) = match following {
		Some(tip) => line(session, Some(tip), None, limit),
		None => (
			head_line
				.iter()
				.take_while({
					let mut ends = 0;
					move |delta| {
						if delta.is_interaction_end() {
							ends += 1;
						}
						ends <= limit
					}
				})
				.cloned()
				.collect::<Vec<_>>(),
			interactions(&head_line).len() > limit,
		),
	};
	let on_line: HashSet<Rev> = undone.iter().chain(&shown).map(|delta| delta.id).collect();

	let view = View {
		children: &children,
		on_line: &on_line,
		head_line: &head_line_revs,
		head,
		people: &people,
		expanded,
		session,
		metadata: gdd.metadata(),
	};
	let mut rows = Vec::new();
	for group in interactions(&undone) {
		rows.push(view.row(&group, true));
	}
	for group in interactions(&shown) {
		rows.push(view.row(&group, false));
	}

	// What has not entered history yet: every author's open transaction first, then each closed transaction
	// waiting for the retirer as a step of its own, the newest first like the rows below.
	let hot_log = session.hot_log();
	let closed = session.closed_transactions();
	let in_closed: HashSet<HotOpId> = closed.iter().flat_map(|transaction| transaction.ops.iter().copied()).collect();
	let mut open_by_peer: Vec<(PeerId, Vec<&RegistryDelta>)> = Vec::new();
	for hot_op in hot_log.iter().filter(|hot_op| !in_closed.contains(&hot_op.id())) {
		match open_by_peer.iter_mut().find(|(peer, _)| *peer == hot_op.timestamp.peer) {
			Some((_, ops)) => ops.push(&hot_op.op),
			None => open_by_peer.push((hot_op.timestamp.peer, vec![&hot_op.op])),
		}
	}
	let mut progress: Vec<HistoryProgressRow> = open_by_peer.into_iter().map(|(peer, ops)| progress_row(&people, peer, &ops, true)).collect();
	for transaction in closed.iter().rev() {
		let ids: HashSet<HotOpId> = transaction.ops.iter().copied().collect();
		let ops: Vec<&RegistryDelta> = hot_log.iter().filter(|hot_op| ids.contains(&hot_op.id())).map(|hot_op| &hot_op.op).collect();
		progress.push(progress_row(&people, transaction.author, &ops, false));
	}

	HistoryPanelState {
		rows,
		progress,
		more,
		following: following.map(|tip| tip.to_string()),
		session: gdd.role().is_some(),
	}
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

/// What one send of the panel is computed against.
struct View<'a> {
	children: &'a HashMap<Rev, Vec<Rev>>,
	on_line: &'a HashSet<Rev>,
	head_line: &'a HashSet<Rev>,
	head: Option<Rev>,
	people: &'a People<'a>,
	expanded: &'a HashSet<Rev>,
	session: &'a Session,
	metadata: &'a HistoryMetadata,
}

impl View<'_> {
	fn row(&self, group: &[&Delta], undone: bool) -> HistoryRow {
		let closing = group[0];
		let person = self.people.person(closing.author);
		let branches = group
			.iter()
			.flat_map(|delta| self.children.get(&delta.id).into_iter().flatten())
			.filter(|child| !self.on_line.contains(child))
			.map(|&child| self.branch(child))
			.collect();
		let is_expanded = self.expanded.contains(&closing.id);
		let details = match is_expanded {
			true => group
				.iter()
				.map(|delta| {
					let author = self.people.person(delta.author);
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
		let given = self.metadata.rev_label(closing.id);
		HistoryRow {
			id: closing.id.to_string(),
			label: given.map(str::to_string).unwrap_or_else(|| summarize(group.iter().map(|delta| &delta.kind))),
			named: given.is_some(),
			tags: self.metadata.rev_tags(closing.id).into_iter().map(str::to_string).collect(),
			author: person.name,
			anonymous: person.anonymous,
			mine: person.mine,
			color: person.color,
			time: group.iter().find_map(|delta| delta.retired_at()).map(|ms| ms as f64),
			deltas: group.len(),
			branches,
			undone,
			abandoned: !undone && !self.head_line.contains(&closing.id),
			head: Some(closing.id) == self.head,
			expanded: is_expanded,
			details,
		}
	}

	/// The branch that starts at `first`: its tip, following first children, and what its newest interaction did.
	fn branch(&self, first: Rev) -> HistoryBranch {
		let mut tip = first;
		while let Some(&next) = self.children.get(&tip).and_then(|children| children.first()) {
			tip = next;
		}
		let (newest, _) = line(self.session, Some(tip), None, 1);
		let label = self
			.metadata
			.rev_label(tip)
			.map(str::to_string)
			.unwrap_or_else(|| interactions(&newest).first().map(|group| summarize(group.iter().map(|delta| &delta.kind))).unwrap_or_default());
		HistoryBranch { id: tip.to_string(), label }
	}
}

fn progress_row(people: &People, peer: PeerId, ops: &[&RegistryDelta], open: bool) -> HistoryProgressRow {
	let person = people.person(peer);
	HistoryProgressRow {
		label: summarize(ops.iter().copied()),
		author: person.name,
		anonymous: person.anonymous,
		mine: person.mine,
		color: person.color,
		ops: ops.len(),
		open,
	}
}

/// One line for a run of changes: the kinds among them, the most frequent first.
fn summarize<'a>(kinds: impl IntoIterator<Item = &'a RegistryDelta>) -> String {
	let kinds: Vec<&RegistryDelta> = kinds.into_iter().collect();
	let mut counts: Vec<(String, usize)> = Vec::new();
	for kind in &kinds {
		// A registration or a marker says nothing about the document; it labels a run only when it is all there is.
		if matches!(kind, RegistryDelta::RegisterPeer { .. } | RegistryDelta::EndTransaction) && kinds.len() > 1 {
			continue;
		}
		let label = describe(kind);
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
