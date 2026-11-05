use super::utility_types::{HistoryBranch, HistoryDeltaRow, HistoryPanelState, HistoryProgressRow, HistoryRow, LineChange};
use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::sync::identity::{anonymous_name, user_color};
use crate::messages::portfolio::sync::now_ms;
use crate::messages::prelude::*;
use document_format::GddV1;
use document_graph_storage::{Delta, HistoryMetadata, HotOpId, PeerId, RegistryDelta, Rev, Session, Subject, UserId, rev_attr};
use peer_transport::RemotePeer;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[derive(ExtractField)]
pub struct HistoryMessageContext<'a> {
	pub documents: &'a mut HashMap<DocumentId, DocumentMessageHandler>,
	pub active_document_id: Option<DocumentId>,
	pub panel_open: bool,
}

/// Sends the History panel the active document's history and turns its actions into head moves.
#[derive(Debug, Default, ExtractField)]
pub struct HistoryMessageHandler {
	ticking: bool,
	expanded: BTreeSet<Rev>,
	limit: usize,
	following: Option<Rev>,
	last_sent: Option<u64>,
}

/// How many interactions each request adds.
const PAGE: usize = 100;

#[message_handler_data]
impl MessageHandler<HistoryMessage, HistoryMessageContext<'_>> for HistoryMessageHandler {
	/// Every change reaches the panel through the next `Tick`, which sends whenever the panel's inputs changed.
	fn process_message(&mut self, message: HistoryMessage, responses: &mut VecDeque<Message>, mut context: HistoryMessageContext) {
		match message {
			HistoryMessage::Refresh => {
				self.last_sent = None;
				if !self.ticking {
					self.ticking = true;
					responses.add(BroadcastMessage::SubscribeEvent {
						on: EventMessage::AnimationFrame,
						send: Box::new(HistoryMessage::Tick.into()),
					});
				}
			}
			HistoryMessage::Tick => {
				if context.panel_open {
					self.send(&context, responses);
				}
			}
			HistoryMessage::Expand { id, expanded } => {
				if let Some(rev) = parse_rev(&id) {
					match expanded {
						true => self.expanded.insert(rev),
						false => self.expanded.remove(&rev),
					};
				}
			}
			HistoryMessage::LoadMore => self.limit = self.limit.max(PAGE) + PAGE,
			HistoryMessage::Follow { id } => self.following = id.as_deref().and_then(parse_rev),
			HistoryMessage::GoBack { id } => {
				let Some((gdd, rev)) = active_storage(&context).zip(parse_rev(&id)) else { return };
				match place(gdd.session(), rev) {
					Some(Place::Undone(steps)) => (0..steps).for_each(|_| responses.add(DocumentMessage::Redo)),
					Some(Place::Line(0)) => {}
					// A session moves the head for everyone at once. Alone, undo each interaction to keep undo in step
					Some(Place::Line(steps)) => match gdd.role().is_some() {
						true => responses.add(DocumentMessage::ChangeHistoryLine { rev, change: LineChange::MoveHead }),
						false => (0..steps).for_each(|_| responses.add(DocumentMessage::Undo)),
					},
					None => log::warn!("The History panel asked to go to {rev:?}, which is not on the head's line"),
				}
			}
			HistoryMessage::RemoveStep { id } => {
				if let Some(rev) = parse_rev(&id) {
					responses.add(DocumentMessage::ChangeHistoryLine { rev, change: LineChange::Remove });
				}
			}
			HistoryMessage::BringBack { id } => {
				let Some((gdd, rev)) = active_storage(&context).zip(parse_rev(&id)) else { return };
				match place(gdd.session(), rev) {
					Some(Place::Undone(steps)) => (0..steps).for_each(|_| responses.add(DocumentMessage::Redo)),
					Some(Place::Line(_)) => {}
					None => responses.add(DocumentMessage::ChangeHistoryLine { rev, change: LineChange::Restore }),
				}
			}
			HistoryMessage::Rename { id, label } => {
				let Some((gdd, rev)) = active_storage_mut(&mut context).zip(parse_rev(&id)) else { return };
				if let Err(error) = gdd.record(Subject::Rev(rev), rev_attr::LABEL, serde_json::Value::from(label.trim()), now_ms()) {
					log::warn!("Naming the step failed: {error}");
				}
			}
			HistoryMessage::Tag { id, tag, on } => {
				let tag = tag.trim();
				let Some((gdd, rev)) = active_storage_mut(&mut context).zip(parse_rev(&id)).filter(|_| !tag.is_empty()) else {
					return;
				};
				if let Err(error) = gdd.record(Subject::Rev(rev), &format!("{}{tag}", rev_attr::TAG_PREFIX), serde_json::Value::Bool(on), now_ms()) {
					log::warn!("Tagging the step failed: {error}");
				}
			}
		}
	}

	advertise_actions!(HistoryMessageDiscriminant;);
}

impl HistoryMessageHandler {
	fn send(&mut self, context: &HistoryMessageContext, responses: &mut VecDeque<Message>) {
		let gdd = active_storage(context);
		if self.following.is_some_and(|tip| gdd.is_none_or(|gdd| gdd.session().delta(tip).is_none())) {
			self.following = None;
		}
		let limit = self.limit.max(PAGE);
		let fingerprint = fingerprint(context.active_document_id, gdd, limit, &self.expanded, self.following);
		if self.last_sent == Some(fingerprint) {
			return;
		}
		self.last_sent = Some(fingerprint);
		let state = gdd.map(|gdd| panel_state(gdd, limit, &self.expanded, self.following)).unwrap_or_default();
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
	/// On the head's line, this many interactions back.
	Line(usize),
	/// Undone above the head, this many redos forward.
	Undone(usize),
}

fn place(session: &Session, rev: Rev) -> Option<Place> {
	let head = session.head_rev();
	let (kept, _) = line(session, head, None, usize::MAX);
	if let Some(steps) = interactions(&kept).iter().position(|group| group[0].id == rev) {
		return Some(Place::Line(steps));
	}
	// The stack's first entry is the farthest undone, so its line holds every undone step
	let checkpoint = *session.redo_stack().first()?;
	let (undone, _) = line(session, Some(checkpoint), head, usize::MAX);
	let groups = interactions(&undone);
	let index = groups.iter().position(|group| group[0].id == rev)?;
	Some(Place::Undone(groups.len() - index))
}

/// A hash of the panel's inputs, cheap enough for every frame.
fn fingerprint(document_id: Option<DocumentId>, gdd: Option<&GddV1>, limit: usize, expanded: &BTreeSet<Rev>, following: Option<Rev>) -> u64 {
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	(document_id, limit, expanded, following).hash(&mut hasher);
	if let Some(gdd) = gdd {
		let session = gdd.session();
		(session.history_len(), session.head_rev(), session.redo_stack(), session.hot_log().len(), gdd.role().is_some()).hash(&mut hasher);
		for remote in gdd.peers() {
			(remote.user, &remote.name).hash(&mut hasher);
		}
		for fact in gdd.metadata().facts() {
			(&fact.subject, &fact.key, &fact.stamp).hash(&mut hasher);
		}
	}
	hasher.finish()
}

fn panel_state(gdd: &GddV1, limit: usize, expanded: &BTreeSet<Rev>, following: Option<Rev>) -> HistoryPanelState {
	let session = gdd.session();
	let people = People {
		session,
		metadata: gdd.metadata(),
		peers: gdd.peers().collect(),
	};

	let mut children: HashMap<Rev, Vec<Rev>> = HashMap::new();
	for delta in session.history() {
		for parent in delta.all_parents() {
			children.entry(parent).or_default().push(delta.id);
		}
	}

	let head = session.head_rev();
	let (head_line, _) = line(session, head, None, usize::MAX);
	let head_line_revs: HashSet<Rev> = head_line.iter().map(|delta| delta.id).collect();

	let undone = match (following, session.redo_stack().first()) {
		(None, Some(&checkpoint)) => line(session, Some(checkpoint), head, usize::MAX).0,
		_ => Vec::new(),
	};
	let (shown, more) = line(session, following.or(head), None, limit);
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
	let undone_rows = interactions(&undone).into_iter().map(|group| view.row(&group, true));
	let shown_rows = interactions(&shown).into_iter().map(|group| view.row(&group, false));
	let rows = undone_rows.chain(shown_rows).collect();

	// Open interactions first, then closed ones waiting to retire, newest first
	let hot_log = session.hot_log();
	let closed = session.closed_interactions();
	let in_closed: HashSet<HotOpId> = closed.iter().flat_map(|interaction| interaction.ops.iter().copied()).collect();
	let mut open_by_peer: Vec<(PeerId, Vec<&RegistryDelta>)> = Vec::new();
	for hot_op in hot_log.iter().filter(|hot_op| !in_closed.contains(&hot_op.id())) {
		match open_by_peer.iter_mut().find(|(peer, _)| *peer == hot_op.timestamp.peer) {
			Some((_, ops)) => ops.push(&hot_op.op),
			None => open_by_peer.push((hot_op.timestamp.peer, vec![&hot_op.op])),
		}
	}
	let mut progress: Vec<HistoryProgressRow> = open_by_peer.into_iter().map(|(peer, ops)| progress_row(&people, peer, &ops, true)).collect();
	for interaction in closed.iter().rev() {
		let ids: HashSet<HotOpId> = interaction.ops.iter().copied().collect();
		let ops: Vec<&RegistryDelta> = hot_log.iter().filter(|hot_op| ids.contains(&hot_op.id())).map(|hot_op| &hot_op.op).collect();
		progress.push(progress_row(&people, interaction.author, &ops, false));
	}

	HistoryPanelState {
		rows,
		progress,
		more,
		following: following.map(|tip| tip.to_string()),
		session: gdd.role().is_some(),
	}
}

/// The deltas from `from` back along first parents to `until` (exclusive) or the root, newest first. Stops after
/// `limit` interaction ends, and the flag says whether it did.
fn line(session: &Session, from: Option<Rev>, until: Option<Rev>, limit: usize) -> (Vec<&Delta>, bool) {
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
		deltas.push(delta);
		current = delta.parent;
	}
	(deltas, false)
}

/// Groups a newest-first line into interactions. Newest deltas without an end, retired mid-interaction, form their own.
fn interactions<'a>(deltas: &[&'a Delta]) -> Vec<Vec<&'a Delta>> {
	let mut groups: Vec<Vec<&Delta>> = Vec::new();
	for &delta in deltas {
		match groups.last_mut() {
			Some(group) if !delta.is_interaction_end() => group.push(delta),
			_ => groups.push(vec![delta]),
		}
	}
	groups
}

struct View<'a> {
	children: &'a HashMap<Rev, Vec<Rev>>,
	on_line: &'a HashSet<Rev>,
	head_line: &'a HashSet<Rev>,
	head: Option<Rev>,
	people: &'a People<'a>,
	expanded: &'a BTreeSet<Rev>,
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

/// One line for a run of changes, most frequent kind first. Peer registrations and markers are named only when alone.
fn summarize<'a>(kinds: impl IntoIterator<Item = &'a RegistryDelta>) -> String {
	let is_bookkeeping = |kind: &RegistryDelta| matches!(kind, RegistryDelta::RegisterPeer { .. } | RegistryDelta::Meta);
	let mut kinds: Vec<&RegistryDelta> = kinds.into_iter().collect();
	if kinds.iter().any(|kind| !is_bookkeeping(kind)) {
		kinds.retain(|kind| !is_bookkeeping(kind));
	}
	let mut counts: Vec<(String, usize)> = Vec::new();
	for kind in kinds {
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
		RegistryDelta::Meta => "Marked a step".into(),
		RegistryDelta::Other(_) => "Changed something".into(),
	}
}

/// Names and colours authors by user id. The document's record wins, then the announced name, then the stand-in.
struct People<'a> {
	session: &'a Session,
	metadata: &'a HistoryMetadata,
	peers: Vec<&'a RemotePeer>,
}

struct Person {
	name: String,
	anonymous: bool,
	mine: bool,
	color: String,
}

impl People<'_> {
	fn person(&self, peer: PeerId) -> Person {
		// A peer that never registered is its own user
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

#[cfg(test)]
mod tests {
	use super::*;

	/// Undoing pushes each step's head, so undone steps must be read from the first entry.
	#[test]
	fn every_undone_step_stays_on_the_panel() {
		use document_container::AnyContainer;
		use document_container::backends::memory::MemoryBackend;
		use document_format::GddV1Layout;
		use document_graph_storage::{Network, NetworkId};

		let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), UserId(1), 1, "editor".into(), "stdlib".into()).expect("create");
		for id in 1..=3 {
			let network = Network::default();
			gdd.stage_ops([RegistryDelta::AddNetwork { id: NetworkId(id), network }]).expect("stage");
			gdd.retire_pending_interaction().expect("retire");
		}
		gdd.undo().expect("undo");
		gdd.undo().expect("undo");

		assert_eq!(panel_state(&gdd, PAGE, &BTreeSet::new(), None).rows.len(), 3);
	}

	#[test]
	fn a_run_of_only_markers_is_named_by_them() {
		let register = RegistryDelta::RegisterPeer { peer: PeerId(1), user: UserId(1) };
		let merge = RegistryDelta::Merge { extra_parents: Vec::new() };
		assert_eq!(summarize([&RegistryDelta::Meta, &RegistryDelta::Meta]), "Marked a step ×2");
		assert_eq!(summarize([&register, &RegistryDelta::Meta]), "Joined the document, Marked a step");
		assert_eq!(summarize([&RegistryDelta::Meta, &merge, &register]), "Joined two lines of work");
	}
}
