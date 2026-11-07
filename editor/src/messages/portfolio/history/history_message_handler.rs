use super::utility_types::{
	DocumentCapture, HistoryBranch, HistoryDeltaRow, HistoryOutline, HistoryPanelState, HistoryProgressRow, HistoryRow, HistoryShot, HistoryThumbnail, LineChange, OutlineChange,
};
use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::TransactionStatus;
use crate::messages::portfolio::sync::identity::{anonymous_name, user_color};
use crate::messages::portfolio::sync::now_ms;
use crate::messages::prelude::*;
use crate::node_graph_executor::NodeGraphExecutor;
use document_format::GddV1;
use document_graph_storage::attr::{delta as delta_attr, network as network_attr, node as node_attr, session as session_attr};
use document_graph_storage::{
	AttributeDelta, Declarations, Delta, HistoryMetadata, HotOp, HotOpId, Implementation, Node, NodeId, PeerId, Registry, RegistryDelta, Rev, Session, Subject, TimeStamp, UserId, Value, rev_attr,
};
use glam::DVec2;
use graph_craft::ProtoNodeIdentifier;
use peer_transport::RemotePeer;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[derive(ExtractField)]
pub struct HistoryMessageContext<'a> {
	pub documents: &'a mut HashMap<DocumentId, DocumentMessageHandler>,
	pub active_document_id: Option<DocumentId>,
	pub panel_open: bool,
	pub executor: &'a NodeGraphExecutor,
}

/// Sends the History panel the active document's history and turns its actions into head moves.
#[derive(Debug, Default, ExtractField)]
pub struct HistoryMessageHandler {
	ticking: bool,
	expanded: BTreeSet<Rev>,
	limit: usize,
	following: Option<Rev>,
	/// The step the active document was last seen previewing, so the panel shows the head's line again once it ends.
	previewed: Option<Rev>,
	last_sent: Option<u64>,
	/// Each document's steps as they looked once done, captured while the panel was open, by the stamp of their newest change.
	captures: HashMap<DocumentId, HashMap<TimeStamp, Capture>>,
	requested: Option<(DocumentId, TimeStamp)>,
}

/// What a captured step showed, without its image, which the frontend keeps.
#[derive(Debug)]
struct Capture {
	frame: [DVec2; 2],
	layers: BTreeMap<graph_craft::document::NodeId, LayerShot>,
	/// Orders the captures so the oldest is dropped first.
	taken: u64,
}

#[derive(Debug, PartialEq)]
struct LayerShot {
	bounds: Option<[DVec2; 2]>,
	/// Changes with what the layer itself shows.
	source: Option<graph_craft::document::NodeId>,
	/// A group or artboard, which changes with its children and is only shown coming and going.
	container: bool,
}

/// How many interactions each request adds.
const PAGE: usize = 100;
/// The most recent captures kept for each document, as many as the frontend keeps images.
const MAX_CAPTURES: usize = 300;

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
					self.request_capture(&context);
					self.send(&context, responses);
				}
			}
			HistoryMessage::Captured { document_id, step, capture } => {
				if self.requested == Some((document_id, step)) {
					self.requested = None;
				}
				if let Some(capture) = capture {
					self.receive_capture(document_id, step, *capture, &context, responses);
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
			HistoryMessage::Preview { id } => responses.add(DocumentMessage::PreviewHistoryStep {
				rev: id.as_deref().and_then(parse_rev),
			}),
			HistoryMessage::RetirePending => {
				let Some(document) = context.active_document_id.and_then(|id| context.documents.get_mut(&id)) else {
					return;
				};
				if !document.is_in_session() && document.network_interface.transaction_status() == TransactionStatus::Finished {
					document.retire_storage_interaction();
				}
			}
			HistoryMessage::GoBack { id } => {
				let Some((gdd, rev)) = active_storage(&context).zip(parse_rev(&id)) else { return };
				responses.add(DocumentMessage::PreviewHistoryStep { rev: None });
				match place(gdd.session(), rev) {
					Some(Place::Undone(steps)) => (0..steps).for_each(|_| responses.add(DocumentMessage::Redo)),
					Some(Place::Line(0)) => {}
					Some(Place::Line(_)) | None => responses.add(DocumentMessage::ChangeHistoryLine { rev, change: LineChange::MoveHead }),
				}
			}
			HistoryMessage::RemoveStep { id } => {
				if let Some(rev) = parse_rev(&id) {
					responses.add(DocumentMessage::ChangeHistoryLine { rev, change: LineChange::Remove });
				}
			}
			HistoryMessage::BringBack { id } => {
				let Some((gdd, rev)) = active_storage(&context).zip(parse_rev(&id)) else { return };
				responses.add(DocumentMessage::PreviewHistoryStep { rev: None });
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
	/// Asks for the newest step to be captured once no tool is mid-gesture, since a capture shows the document as it renders now.
	fn request_capture(&mut self, context: &HistoryMessageContext) {
		self.captures.retain(|id, _| context.documents.contains_key(id));
		let Some(document_id) = context.active_document_id else { return };
		let Some(document) = context.documents.get(&document_id) else { return };
		if document.network_interface.transaction_status() != TransactionStatus::Finished {
			return;
		}
		let Some(step) = document.storage().and_then(|gdd| current_stamp(gdd.session())) else { return };
		let captured = self.captures.get(&document_id).is_some_and(|captures| captures.contains_key(&step));
		if captured || self.requested == Some((document_id, step)) {
			return;
		}
		if context.executor.submit_document_capture(document, document_id, step) {
			self.requested = Some((document_id, step));
		}
	}

	/// Keeps a capture that still shows its step, with where each layer was, and gives the frontend its image.
	fn receive_capture(&mut self, document_id: DocumentId, step: TimeStamp, capture: DocumentCapture, context: &HistoryMessageContext, responses: &mut VecDeque<Message>) {
		let Some(document) = context.documents.get(&document_id) else { return };
		if document.storage().and_then(|gdd| current_stamp(gdd.session())) != Some(step) {
			return;
		}

		let metadata = document.metadata();
		let layers = metadata
			.all_layers()
			.map(|layer| {
				let node = layer.to_node();
				let shot = LayerShot {
					bounds: metadata.bounding_box_document(layer),
					source: capture.layer_sources.get(&node).copied(),
					container: layer.has_children(metadata) || document.network_interface.is_artboard(&node, &[]),
				};
				(node, shot)
			})
			.collect();
		let captures = self.captures.entry(document_id).or_default();
		let taken = captures.values().map(|capture| capture.taken + 1).max().unwrap_or(0);
		if captures.len() >= MAX_CAPTURES
			&& let Some(&oldest) = captures.iter().min_by_key(|(_, capture)| capture.taken).map(|(rev, _)| rev)
		{
			captures.remove(&oldest);
		}
		captures.insert(step, Capture { frame: capture.frame, layers, taken });
		responses.add(FrontendMessage::UpdateHistoryThumbnail { id: shot_id(step), svg: capture.svg });
	}

	fn send(&mut self, context: &HistoryMessageContext, responses: &mut VecDeque<Message>) {
		let document = context.active_document_id.and_then(|id| context.documents.get(&id));
		let gdd = document.and_then(|document| document.storage());
		if self.following.is_some_and(|tip| gdd.is_none_or(|gdd| gdd.session().delta(tip).is_none())) {
			self.following = None;
		}
		let previewing = document.and_then(|document| document.previewed_history_step());
		if self.previewed.is_some() && previewing.is_none() {
			self.following = None;
		}
		self.previewed = previewing;
		let limit = self.limit.max(PAGE);
		let captures = context.active_document_id.and_then(|id| self.captures.get(&id));
		// The newest capture's order changes with every capture, even once the oldest are dropped
		let newest_capture = captures.and_then(|captures| captures.values().map(|capture| capture.taken + 1).max()).unwrap_or(0);
		let fingerprint = fingerprint(context.active_document_id, gdd, limit, &self.expanded, self.following, previewing, newest_capture);
		if self.last_sent == Some(fingerprint) {
			return;
		}
		self.last_sent = Some(fingerprint);
		let state = document
			.zip(gdd)
			.map(|(document, gdd)| HistoryPanelState {
				previewing: previewing.map(|rev| rev.to_string()),
				..panel_state(gdd, document.declarations(), captures, limit, &self.expanded, self.following)
			})
			.unwrap_or_default();
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
pub(crate) enum Place {
	/// On the head's line, this many interactions back.
	Line(usize),
	/// Undone above the head, this many redos forward.
	Undone(usize),
}

/// Where `rev` is relative to the head. `None` for a step on a branch.
pub(crate) fn place(session: &Session, rev: Rev) -> Option<Place> {
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
fn fingerprint(document_id: Option<DocumentId>, gdd: Option<&GddV1>, limit: usize, expanded: &BTreeSet<Rev>, following: Option<Rev>, previewing: Option<Rev>, newest_capture: u64) -> u64 {
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	(document_id, limit, expanded, following, previewing, newest_capture).hash(&mut hasher);
	if let Some(gdd) = gdd {
		let session = gdd.session();
		(session.history_len(), session.head_rev(), session.redo_stack(), session.hot_log().len(), session.pending_action()).hash(&mut hasher);
		for remote in gdd.peers() {
			(remote.user, &remote.name).hash(&mut hasher);
		}
		for fact in gdd.metadata().facts() {
			(&fact.subject, &fact.key, &fact.stamp).hash(&mut hasher);
		}
	}
	hasher.finish()
}

fn panel_state(gdd: &GddV1, declarations: &Declarations, captures: Option<&HashMap<TimeStamp, Capture>>, limit: usize, expanded: &BTreeSet<Rev>, following: Option<Rev>) -> HistoryPanelState {
	let session = gdd.session();
	let names = Names {
		registry: gdd.registry(),
		declarations,
	};
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
		names: &names,
		captures,
		expanded,
		session,
		metadata: gdd.metadata(),
	};
	let undone_rows = interactions(&undone).into_iter().map(|group| view.row(&group, true));
	let shown_rows = interactions(&shown).into_iter().map(|group| view.row(&group, false));
	let rows = undone_rows.chain(shown_rows).collect();

	let collaborating = gdd.peers().next().is_some();
	HistoryPanelState {
		rows,
		progress: progress_rows(session, &people, &names, captures),
		more,
		following: following.map(|tip| tip.to_string()),
		previewing: None,
		collaborating,
	}
}

/// Open interactions first, then closed ones waiting to retire, newest first. Each is compared with the step whose newest
/// change came last before its own.
fn progress_rows(session: &Session, people: &People, names: &Names, captures: Option<&HashMap<TimeStamp, Capture>>) -> Vec<HistoryProgressRow> {
	let hot_log = session.hot_log();
	let closed = session.closed_interactions();
	let in_closed: HashSet<HotOpId> = closed.iter().flat_map(|interaction| interaction.ops.iter().copied()).collect();
	let mut open_by_peer: Vec<(PeerId, Vec<&HotOp>)> = Vec::new();
	for hot_op in hot_log.iter().filter(|hot_op| !in_closed.contains(&hot_op.id())) {
		match open_by_peer.iter_mut().find(|(peer, _)| *peer == hot_op.timestamp.peer) {
			Some((_, ops)) => ops.push(hot_op),
			None => open_by_peer.push((hot_op.timestamp.peer, vec![hot_op])),
		}
	}
	let mut steps: Vec<(PeerId, Vec<&HotOp>, bool, Option<String>)> = open_by_peer
		.into_iter()
		.map(|(peer, ops)| {
			let action = session.pending_action().filter(|_| peer == session.peer()).map(str::to_string);
			(peer, ops, true, action)
		})
		.collect();
	for interaction in closed.iter().rev() {
		let ids: HashSet<HotOpId> = interaction.ops.iter().copied().collect();
		let ops: Vec<&HotOp> = hot_log.iter().filter(|hot_op| ids.contains(&hot_op.id())).collect();
		let action = ops.iter().find_map(|hot_op| hot_op.attributes.get_typed::<String>(delta_attr::ACTION));
		steps.push((interaction.author, ops, false, action));
	}

	let stamps: Vec<Option<TimeStamp>> = steps.iter().map(|(_, ops, ..)| newest_change(ops.iter().copied())).collect();
	let head = head_stamp(session);
	steps
		.into_iter()
		.zip(&stamps)
		.map(|((peer, ops, open, action), &stamp)| {
			let thumbnail = stamp.and_then(|stamp| {
				let before = stamps.iter().flatten().copied().filter(|&other| other < stamp).max().or(head);
				thumbnail(captures?, stamp, before)
			});
			let kinds: Vec<&RegistryDelta> = ops.iter().map(|hot_op| &hot_op.op).collect();
			progress_row(people, names, peer, &kinds, open, action, thumbnail)
		})
		.collect()
}

/// The stamp of the newest change among `ops`, which keys a step's capture. Retiring keeps each op's stamp and merging
/// keeps a step's newest op, so a step keeps its key from the hot log into history.
fn newest_change<'a>(ops: impl IntoIterator<Item = &'a HotOp>) -> Option<TimeStamp> {
	ops.into_iter().filter(|hot_op| !matches!(hot_op.op, RegistryDelta::Meta)).map(|hot_op| hot_op.timestamp).max()
}

fn head_stamp(session: &Session) -> Option<TimeStamp> {
	session.head_rev().and_then(|rev| session.delta(rev)).map(|delta| delta.timestamp)
}

/// The key of the step the document shows now: the newest change still hot, else the head.
fn current_stamp(session: &Session) -> Option<TimeStamp> {
	newest_change(session.hot_log()).or_else(|| head_stamp(session))
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
	names: &'a Names<'a>,
	captures: Option<&'a HashMap<TimeStamp, Capture>>,
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
		let shown = foreground(group, |delta| &delta.kind);
		let details = match is_expanded {
			true => shown
				.iter()
				.map(|delta| {
					let author = self.people.person(delta.author);
					HistoryDeltaRow {
						id: delta.id.to_string(),
						label: self.names.describe(&delta.kind),
						author: author.name,
						mine: author.mine,
						time: delta.retired_at().map(|ms| ms as f64),
					}
				})
				.collect(),
			false => Vec::new(),
		};
		let given = self.metadata.rev_label(closing.id);
		HistoryRow {
			id: closing.id.to_string(),
			label: self.label(group),
			named: given.is_some(),
			tags: self.metadata.rev_tags(closing.id).into_iter().map(str::to_string).collect(),
			author: person.name,
			anonymous: person.anonymous,
			mine: person.mine,
			color: person.color,
			time: group.iter().find_map(|delta| delta.retired_at()).map(|ms| ms as f64),
			deltas: shown.len() as u32,
			branches,
			undone,
			abandoned: !undone && !self.head_line.contains(&closing.id),
			head: Some(closing.id) == self.head,
			expanded: is_expanded,
			details,
			thumbnail: self.thumbnail(group),
		}
	}

	/// The step as captured, compared with the step before it.
	fn thumbnail(&self, group: &[&Delta]) -> Option<HistoryThumbnail> {
		let before = group.last()?.parent.and_then(|rev| self.session.delta(rev)).map(|delta| delta.timestamp);
		thumbnail(self.captures?, group[0].timestamp, before)
	}

	fn branch(&self, first: Rev) -> HistoryBranch {
		let mut tip = first;
		while let Some(&next) = self.children.get(&tip).and_then(|children| children.first()) {
			tip = next;
		}
		let (newest, _) = line(self.session, Some(tip), None, 1);
		let label = interactions(&newest).first().map(|group| self.label(group)).unwrap_or_default();
		HistoryBranch { id: tip.to_string(), label }
	}

	/// The name someone gave the step, else what its tool did, else "New Document" for the first step, else a summary of its changes.
	fn label(&self, group: &[&Delta]) -> String {
		let closing = group[0];
		let starts_document = group.last().is_some_and(|oldest| oldest.parent.is_none());
		let named = self.metadata.rev_label(closing.id).or_else(|| closing.action()).or_else(|| starts_document.then_some("New Document"));
		named.map(str::to_string).unwrap_or_else(|| self.names.summarize(group.iter().map(|delta| &delta.kind)))
	}
}

/// The step captured at `after`, compared with the capture at `before` when there is one.
fn thumbnail(captures: &HashMap<TimeStamp, Capture>, after: TimeStamp, before: Option<TimeStamp>) -> Option<HistoryThumbnail> {
	let after_capture = captures.get(&after)?;
	let Some((before, before_capture)) = before.and_then(|stamp| Some((stamp, captures.get(&stamp)?))) else {
		return Some(HistoryThumbnail {
			after: shot(after, after_capture, Vec::new()),
			before: None,
		});
	};
	let (removed, added) = compare(before_capture, after_capture);
	Some(HistoryThumbnail {
		after: shot(after, after_capture, added),
		before: Some(shot(before, before_capture, removed)),
	})
}

/// The ID the frontend keeps a capture's image by.
fn shot_id(stamp: TimeStamp) -> String {
	format!("{}-{}", stamp.counter, stamp.peer.0)
}

fn shot(stamp: TimeStamp, capture: &Capture, outlines: Vec<HistoryOutline>) -> HistoryShot {
	let [min, max] = capture.frame;
	HistoryShot {
		id: shot_id(stamp),
		frame: [min.x, min.y, max.x - min.x, max.y - min.y],
		outlines,
	}
}

/// Outlines what went or changed on the earlier capture, and what came or changed on the later one. A container is
/// outlined only coming or going, since its children's outlines show what changed within it.
fn compare(before: &Capture, after: &Capture) -> (Vec<HistoryOutline>, Vec<HistoryOutline>) {
	let mut on_before = Vec::new();
	let mut on_after = Vec::new();
	for (layer, now) in &after.layers {
		match before.layers.get(layer) {
			None => on_after.extend(outline(OutlineChange::Added, now.bounds)),
			Some(then) if !now.container && then != now => {
				on_before.extend(outline(OutlineChange::Changed, then.bounds));
				on_after.extend(outline(OutlineChange::Changed, now.bounds));
			}
			Some(_) => {}
		}
	}
	for (layer, then) in &before.layers {
		if !after.layers.contains_key(layer) {
			on_before.extend(outline(OutlineChange::Removed, then.bounds));
		}
	}
	(on_before, on_after)
}

fn outline(change: OutlineChange, bounds: Option<[DVec2; 2]>) -> Option<HistoryOutline> {
	let [min, max] = bounds?;
	Some(HistoryOutline {
		change,
		x: min.x,
		y: min.y,
		width: max.x - min.x,
		height: max.y - min.y,
	})
}

fn progress_row(people: &People, names: &Names, peer: PeerId, ops: &[&RegistryDelta], open: bool, action: Option<String>, thumbnail: Option<HistoryThumbnail>) -> HistoryProgressRow {
	let person = people.person(peer);
	HistoryProgressRow {
		label: action.unwrap_or_else(|| names.summarize(ops.iter().copied())),
		author: person.name,
		anonymous: person.anonymous,
		mine: person.mine,
		color: person.color,
		ops: foreground(ops, |kind| kind).len() as u32,
		open,
		thumbnail,
	}
}

/// Whether an op only comes along with an edit: registrations, markers, resources, and attributes that copy a
/// declaration or hold bookkeeping.
fn is_background(kind: &RegistryDelta) -> bool {
	match kind {
		RegistryDelta::RegisterPeer { .. }
		| RegistryDelta::Meta
		| RegistryDelta::AddResource { .. }
		| RegistryDelta::SetResourceHash { .. }
		| RegistryDelta::RemoveResource { .. }
		| RegistryDelta::AddSource { .. }
		| RegistryDelta::RemoveSource { .. }
		| RegistryDelta::ChangeNodeInputAttribute { .. } => true,
		RegistryDelta::ChangeNodeAttribute { delta, .. } => ![
			node_attr::ui::POSITION,
			node_attr::ui::IS_LAYER,
			node_attr::ui::DISPLAY_NAME,
			node_attr::ui::LOCKED,
			node_attr::ui::PINNED,
			node_attr::VISIBLE,
		]
		.contains(&delta.key.as_str()),
		RegistryDelta::ChangeNetworkAttribute { delta, .. } => ![session_attr::network::PREVIEWING, network_attr::PINNED_ORDER].contains(&delta.key.as_str()),
		_ => false,
	}
}

/// The ops worth showing, or all of them when every one is background. Setting up a node or subgraph added in the
/// same run is part of adding it.
fn foreground<'a, T>(items: &'a [T], kind: impl Fn(&T) -> &RegistryDelta) -> Vec<&'a T> {
	let mut added_nodes = HashSet::new();
	let mut added_networks = HashSet::new();
	let mut implemented = HashSet::new();
	for item in items {
		match kind(item) {
			RegistryDelta::AddNode { id, node } => {
				added_nodes.insert(*id);
				if let Implementation::Network(network) = node.implementation() {
					implemented.insert(*network);
				}
			}
			RegistryDelta::AddNetwork { id, .. } => _ = added_networks.insert(*id),
			_ => {}
		}
	}
	let setup = |op: &RegistryDelta| match op {
		RegistryDelta::AddNode { node, .. } => added_networks.contains(&node.network()),
		RegistryDelta::AddNetwork { id, .. } => implemented.contains(id),
		RegistryDelta::ChangeNodeInput { id, .. }
		| RegistryDelta::SetNodeInputs { id, .. }
		| RegistryDelta::SetNodeImplementation { id, .. }
		| RegistryDelta::ChangeNodeAttribute { id, .. }
		| RegistryDelta::ChangeNodeInputAttribute { id, .. } => added_nodes.contains(id),
		RegistryDelta::SetNetworkExport { id, .. } | RegistryDelta::ChangeNetworkAttribute { id, .. } => added_networks.contains(id),
		_ => false,
	};
	let shown: Vec<&T> = items.iter().filter(|item| !is_background(kind(item)) && !setup(kind(item))).collect();
	if shown.is_empty() { items.iter().collect() } else { shown }
}

/// Names nodes and inputs as the document has them now, so a node removed since goes unnamed.
struct Names<'a> {
	registry: &'a Registry,
	declarations: &'a Declarations,
}

impl Names<'_> {
	fn node(&self, node: &Node) -> String {
		let reference = || match node.implementation() {
			Implementation::Network(network) => self.registry.networks.get(network)?.attributes.get_typed::<String>(node_attr::ui::REFERENCE),
			Implementation::ProtoNode(declaration) => {
				let identifier = ProtoNodeIdentifier::with_owned_string(self.declarations.get(declaration)?.identifier.clone());
				Some(DefinitionIdentifier::ProtoNode(identifier).implementation_name_from_identifier())
			}
		};
		node.attributes().get_typed::<String>(node_attr::ui::DISPLAY_NAME).or_else(reference).unwrap_or_else(|| "a Node".into())
	}

	fn node_by_id(&self, id: NodeId) -> String {
		self.registry.node_instances.get(&id).map(|node| self.node(node)).unwrap_or_else(|| "a Node".into())
	}

	fn input(&self, id: NodeId, index: u32) -> String {
		let slot = self.registry.node_instances.get(&id).and_then(|node| node.inputs().get(index as usize));
		slot.and_then(|slot| slot.attributes.get_typed::<String>(node_attr::input::ui::NAME))
			.map(|name| name.trim().to_string())
			.unwrap_or_else(|| "a Parameter".into())
	}

	/// One line for a run of changes, nodes added or removed first since the rest follows from them, then the most frequent kind.
	fn summarize<'a>(&self, kinds: impl IntoIterator<Item = &'a RegistryDelta>) -> String {
		let kinds: Vec<&RegistryDelta> = kinds.into_iter().collect();
		let mut counts: Vec<(String, usize, bool)> = Vec::new();
		for kind in foreground(&kinds, |kind| kind) {
			let label = self.describe(kind);
			let structural = matches!(kind, RegistryDelta::AddNode { .. } | RegistryDelta::RemoveNode { .. });
			match counts.iter_mut().find(|(known, ..)| *known == label) {
				Some((_, count, known_structural)) => {
					*count += 1;
					*known_structural |= structural;
				}
				None => counts.push((label, 1, structural)),
			}
		}
		counts.sort_by_key(|(_, count, structural)| (!structural, std::cmp::Reverse(*count)));
		let mut parts: Vec<String> = counts
			.iter()
			.take(2)
			.map(|(label, count, _)| match count {
				1 => label.clone(),
				count => format!("{label} ×{count}"),
			})
			.collect();
		if counts.len() > 2 {
			parts.push(format!("{} More", counts.len() - 2));
		}
		parts.join(", ")
	}

	fn describe(&self, kind: &RegistryDelta) -> String {
		let toggled = |delta: &AttributeDelta, on: &str, off: &str, id: NodeId| {
			let verb = if matches!(delta.value, Some(Value::Bool(true))) { on } else { off };
			format!("{verb} {}", self.node_by_id(id))
		};
		match kind {
			RegistryDelta::AddNode { node, .. } => format!("Added {}", self.node(node)),
			RegistryDelta::RemoveNode { snapshot, .. } => format!("Removed {}", self.node(snapshot)),
			RegistryDelta::ChangeNodeInput { id, index, .. } => format!("Changed {} of {}", self.input(*id, *index), self.node_by_id(*id)),
			RegistryDelta::SetNodeInputs { id, .. } => format!("Changed the Parameters of {}", self.node_by_id(*id)),
			RegistryDelta::SetNodeImplementation { id, .. } => format!("Changed What {} Does", self.node_by_id(*id)),
			RegistryDelta::ChangeNodeAttribute { id, delta } => match delta.key.as_str() {
				node_attr::ui::POSITION => format!("Moved {} in the Graph", self.node_by_id(*id)),
				node_attr::ui::DISPLAY_NAME => match &delta.value {
					Some(Value::Str(name)) => {
						let layer = self.registry.node_instances.get(id).and_then(|node| node.attributes().get_typed::<bool>(node_attr::ui::IS_LAYER));
						let term = if layer == Some(true) { "Layer" } else { "Node" };
						format!("Renamed a {term} to {name}")
					}
					_ => format!("Reset the Name of {}", self.node_by_id(*id)),
				},
				node_attr::ui::LOCKED => toggled(delta, "Locked", "Unlocked", *id),
				node_attr::ui::PINNED => toggled(delta, "Pinned", "Unpinned", *id),
				node_attr::ui::IS_LAYER => toggled(delta, "Made a Layer of", "Made a Node of", *id),
				node_attr::VISIBLE => toggled(delta, "Showed", "Hid", *id),
				key => format!("Set Node {key}"),
			},
			RegistryDelta::ChangeNodeInputAttribute { delta, .. } => format!("Set Parameter {}", delta.key),
			RegistryDelta::SetNetworkExport { .. } => "Changed an Output".into(),
			RegistryDelta::ChangeNetworkAttribute { delta, .. } => match delta.key.as_str() {
				session_attr::network::PREVIEWING => "Changed the Previewed Node".into(),
				network_attr::PINNED_ORDER => "Reordered the Pinned Nodes".into(),
				key => format!("Set Subgraph {key}"),
			},
			RegistryDelta::AddNetwork { .. } => "Added a Subgraph".into(),
			RegistryDelta::RemoveNetwork { .. } => "Removed a Subgraph".into(),
			RegistryDelta::AddResource { .. } => "Added a Resource".into(),
			RegistryDelta::SetResourceHash { .. } => "Updated a Resource".into(),
			RegistryDelta::RemoveResource { .. } => "Removed a Resource".into(),
			RegistryDelta::AddSource { .. } | RegistryDelta::RemoveSource { .. } => "Changed Where a Resource Comes From".into(),
			RegistryDelta::RegisterPeer { .. } => "Joined the Document".into(),
			RegistryDelta::ChangeDocumentAttribute { delta } => match delta.key.as_str() {
				session_attr::doc::PTZ => "Moved the View".into(),
				session_attr::doc::RENDER_MODE => "Changed the View Mode".into(),
				session_attr::doc::OVERLAYS => "Changed the Overlays".into(),
				session_attr::doc::RULERS_VISIBLE => "Showed or Hid the Rulers".into(),
				session_attr::doc::SNAPPING => "Changed Snapping".into(),
				session_attr::doc::COLLAPSED => "Expanded or Collapsed Layers".into(),
				key => format!("Set Document {key}"),
			},
			RegistryDelta::Merge { .. } => "Joined Two Lines of Work".into(),
			RegistryDelta::Meta => "Marked a Step".into(),
			RegistryDelta::Other(_) => "Changed Something".into(),
		}
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

		assert_eq!(panel_state(&gdd, &Declarations::new(), None, PAGE, &BTreeSet::new(), None).rows.len(), 3);
	}

	#[test]
	fn a_run_of_only_markers_is_named_by_them() {
		let registry = Registry::default();
		let names = Names {
			registry: &registry,
			declarations: &Declarations::new(),
		};
		let summarize = |kinds: &[&RegistryDelta]| names.summarize(kinds.iter().copied());
		let register = RegistryDelta::RegisterPeer { peer: PeerId(1), user: UserId(1) };
		let merge = RegistryDelta::Merge { extra_parents: Vec::new() };
		assert_eq!(summarize(&[&RegistryDelta::Meta, &RegistryDelta::Meta]), "Marked a Step ×2");
		assert_eq!(summarize(&[&register, &RegistryDelta::Meta]), "Joined the Document, Marked a Step");
		assert_eq!(summarize(&[&RegistryDelta::Meta, &merge, &register]), "Joined Two Lines of Work");
	}

	#[test]
	fn copied_declaration_details_stay_out_of_a_name() {
		let registry = Registry::default();
		let names = Names {
			registry: &registry,
			declarations: &Declarations::new(),
		};
		let attribute = |key: &str| AttributeDelta {
			key: key.to_string(),
			value: Some(Value::Str("copied".into())),
		};
		let id = NodeId(1);
		let description = RegistryDelta::ChangeNodeInputAttribute {
			id,
			index: 0,
			delta: attribute(node_attr::input::ui::DESCRIPTION),
		};
		let output_names = RegistryDelta::ChangeNodeAttribute {
			id,
			delta: attribute(node_attr::ui::OUTPUT_NAMES),
		};
		let moved = RegistryDelta::ChangeNodeAttribute {
			id,
			delta: attribute(node_attr::ui::POSITION),
		};
		assert_eq!(names.summarize([&description, &output_names, &description, &moved]), "Moved a Node in the Graph");
	}

	#[test]
	fn a_comparison_outlines_what_came_went_and_changed() {
		use graph_craft::document::NodeId as LayerId;

		let layer = |x: f64, source: u64, container: bool| LayerShot {
			bounds: Some([DVec2::splat(x), DVec2::splat(x + 1.)]),
			source: Some(LayerId(source)),
			container,
		};
		let capture = |layers: Vec<(u64, LayerShot)>| Capture {
			frame: [DVec2::ZERO, DVec2::ONE],
			layers: layers.into_iter().map(|(id, shot)| (LayerId(id), shot)).collect(),
			taken: 0,
		};
		let before = capture(vec![(1, layer(0., 1, false)), (2, layer(5., 2, false)), (3, layer(9., 3, false)), (4, layer(0., 4, true))]);
		let after = capture(vec![(1, layer(0., 1, false)), (2, layer(6., 2, false)), (4, layer(0., 5, true)), (5, layer(3., 6, false))]);

		let (on_before, on_after) = compare(&before, &after);
		let changes = |outlines: Vec<HistoryOutline>| outlines.into_iter().map(|outline| (outline.change, outline.x)).collect::<Vec<_>>();
		assert_eq!(
			changes(on_before),
			[(OutlineChange::Changed, 5.), (OutlineChange::Removed, 9.)],
			"the moved layer's old place and the removed one"
		);
		assert_eq!(
			changes(on_after),
			[(OutlineChange::Changed, 6.), (OutlineChange::Added, 3.)],
			"a container changes with its children and is not outlined"
		);
	}

	#[tokio::test]
	async fn a_step_still_hot_is_captured_once_the_runtime_has_run() {
		use crate::test_utils::test_prelude::*;
		use glam::UVec2;
		use graphene_std::application_io::TimingInformation;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 10., 10.).await;
		let gdd = editor.active_document().storage().expect("every document has a working copy");
		let step = current_stamp(gdd.session()).expect("the rectangle is a step");

		let portfolio = editor.portfolio_message_handler_mut();
		let document_id = portfolio.active_document_id.unwrap();
		let document = portfolio.documents.get_mut(&document_id).unwrap();
		portfolio
			.executor
			.submit_node_graph_evaluation(document, document_id, UVec2::ONE, 1., TimingInformation::default(), Vec::new(), true, DVec2::ZERO)
			.unwrap();
		assert!(portfolio.executor.submit_document_capture(document, document_id, step), "the runtime holds this document's graph");
		editor.runtime.update();
		editor.runtime.run().await;

		let responses = editor.dispatch(Message::Wake);
		let image = responses.iter().find_map(|message| match message {
			FrontendMessage::UpdateHistoryThumbnail { id, svg } if *id == shot_id(step) => Some(svg),
			_ => None,
		});
		assert!(image.is_some_and(|svg| svg.contains("<path")), "the step's capture should reach the frontend");
	}

	#[tokio::test]
	async fn deleting_the_only_layer_is_captured() {
		use crate::test_utils::test_prelude::*;
		use glam::UVec2;
		use graphene_std::application_io::TimingInformation;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.handle_message(DocumentMessage::DeleteSelectedLayers).await;
		editor.handle_message(DocumentMessage::StartTransaction).await;

		let document = editor.active_document();
		let gdd = document.storage().expect("every document has a working copy");
		let step = current_stamp(gdd.session()).expect("the deletion is a step");

		let portfolio = editor.portfolio_message_handler_mut();
		let document_id = portfolio.active_document_id.unwrap();
		let document = portfolio.documents.get_mut(&document_id).unwrap();
		portfolio
			.executor
			.submit_node_graph_evaluation(document, document_id, UVec2::ONE, 1., TimingInformation::default(), Vec::new(), true, DVec2::ZERO)
			.unwrap();
		assert!(portfolio.executor.submit_document_capture(document, document_id, step), "the runtime holds this document's graph");
		editor.runtime.update();
		editor.runtime.run().await;

		let responses = editor.dispatch(Message::Wake);
		let captured = responses
			.iter()
			.any(|message| matches!(message, FrontendMessage::UpdateHistoryThumbnail { id, .. } if *id == shot_id(step)));
		assert!(captured, "an empty document is captured too");
	}

	#[tokio::test]
	async fn a_capture_taken_while_hot_stays_with_its_step_in_history() {
		use crate::test_utils::test_prelude::*;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let document = editor.active_document();
		let gdd = document.storage().expect("every document has a working copy");
		let step = current_stamp(gdd.session()).expect("the rectangle is a step");
		let capture = Capture {
			frame: [DVec2::ZERO, DVec2::ONE],
			layers: BTreeMap::new(),
			taken: 0,
		};
		let captures = HashMap::from([(step, capture)]);
		let state = panel_state(gdd, document.declarations(), Some(&captures), PAGE, &BTreeSet::new(), None);
		assert!(state.progress[0].thumbnail.is_some(), "the step shows its capture before it enters history");

		let gdd = editor.active_document_mut().storage_mut().expect("every document has a working copy");
		gdd.retire_pending_interaction().expect("retire");
		let document = editor.active_document();
		let gdd = document.storage().expect("every document has a working copy");
		let state = panel_state(gdd, document.declarations(), Some(&captures), PAGE, &BTreeSet::new(), None);
		assert!(state.progress.is_empty());
		assert_eq!(
			state.rows[0].thumbnail.as_ref().map(|thumbnail| thumbnail.after.id.clone()),
			Some(shot_id(step)),
			"and keeps it once it has"
		);
	}

	#[tokio::test]
	async fn a_tool_names_its_step() {
		use crate::test_utils::test_prelude::*;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Select, 50., 50., 80., 80., ModifierKeys::empty()).await;

		let gdd = editor.active_document_mut().storage_mut().expect("every document has a working copy");
		gdd.retire_pending_interaction().expect("retire");
		let actions: Vec<&str> = gdd.session().history().filter_map(|delta| delta.action()).collect();
		assert_eq!(actions, ["Drew a Rectangle", "Moved a Layer"]);
	}

	#[tokio::test]
	async fn a_step_shows_before_it_enters_history() {
		use crate::test_utils::test_prelude::*;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Rectangle, 120., 0., 200., 100., ModifierKeys::empty()).await;

		let document = editor.active_document();
		let gdd = document.storage().expect("every document has a working copy");
		let state = panel_state(gdd, document.declarations(), None, PAGE, &BTreeSet::new(), None);
		let labels: Vec<&str> = state.progress.iter().map(|row| row.label.as_str()).collect();
		assert_eq!(labels, ["Drew a Rectangle", "Drew a Rectangle"], "the open draw and the closed one waiting to retire");
	}

	#[tokio::test]
	async fn deleting_a_layer_names_its_step() {
		use crate::test_utils::test_prelude::*;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.handle_message(DocumentMessage::DeleteSelectedLayers).await;
		editor.handle_message(DocumentMessage::StartTransaction).await;

		let document = editor.active_document();
		let gdd = document.storage().expect("every document has a working copy");
		let state = panel_state(gdd, document.declarations(), None, PAGE, &BTreeSet::new(), None);
		let labels: Vec<&str> = state.progress.iter().map(|row| row.label.as_str()).collect();
		assert_eq!(labels, ["Deleted a Layer", "Drew a Rectangle"]);
	}

	#[tokio::test]
	async fn a_pasted_image_names_its_step() {
		use crate::test_utils::test_prelude::*;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		use base64::Engine;

		let png = base64::engine::general_purpose::STANDARD
			.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
			.unwrap();
		let insert = DocumentMessage::InsertImage {
			name: None,
			data: png.into(),
			size: glam::UVec2::ONE,
			mouse: None,
			parent_and_insert_index: None,
			place_at_origin: false,
		};
		editor.handle_message(insert).await;
		editor.handle_message(DocumentMessage::StartTransaction).await;

		let document = editor.active_document();
		let gdd = document.storage().expect("every document has a working copy");
		let state = panel_state(gdd, document.declarations(), None, PAGE, &BTreeSet::new(), None);
		let labels: Vec<&str> = state.progress.iter().map(|row| row.label.as_str()).collect();
		assert!(labels.contains(&"Added an Image"), "{labels:?}");
	}

	/// Bringing a deletion back onto a stack that grew on the deleted layer since removes the layer and reconnects the
	/// newer one to what was below it, rather than leaving it on nothing.
	#[tokio::test]
	async fn bringing_a_deletion_back_onto_a_grown_stack_reconnects_it() {
		use crate::test_utils::test_prelude::*;

		async fn close_step(editor: &mut EditorTestUtils) {
			editor.handle_message(DocumentMessage::StartTransaction).await;
			editor.handle_message(DocumentMessage::EndTransaction).await;
			editor.active_document_mut().retire_storage_interaction();
		}
		fn panel_rows(editor: &EditorTestUtils, following: Option<Rev>) -> Vec<HistoryRow> {
			let document = editor.active_document();
			let gdd = document.storage().expect("every document has a working copy");
			panel_state(gdd, document.declarations(), None, PAGE, &BTreeSet::new(), following).rows
		}

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Ellipse, 150., 0., 250., 100., ModifierKeys::empty()).await;
		close_step(&mut editor).await;
		let ellipse = editor
			.active_document()
			.network_interface
			.selected_nodes()
			.selected_layers(editor.active_document().metadata())
			.next()
			.expect("the ellipse is selected");
		editor.handle_message(NodeGraphMessage::DeleteSelectedNodes { delete_children: true }).await;
		close_step(&mut editor).await;

		let rows = panel_rows(&editor, None);
		editor.handle_message(HistoryMessage::GoBack { id: rows[1].id.clone() }).await;
		editor.drag_tool(ToolType::Rectangle, 300., 0., 350., 50., ModifierKeys::empty()).await;
		close_step(&mut editor).await;

		let branch = panel_rows(&editor, None).into_iter().flat_map(|row| row.branches).next().expect("the deletion is left on a branch");
		let deletion = panel_rows(&editor, parse_rev(&branch.id)).into_iter().find(|row| row.abandoned).expect("the deletion is off the line");
		editor.handle_message(HistoryMessage::BringBack { id: deletion.id }).await;

		let document = editor.active_document();
		let layers: Vec<_> = document.metadata().all_layers().collect();
		assert!(!layers.contains(&ellipse), "the deletion took the ellipse");
		assert_eq!(layers.len(), 2, "both rectangles are still drawn: {layers:?}");
		let violations = document.network_interface.validate_invariants();
		assert!(violations.is_empty(), "{violations:#?}");
	}

	async fn three_rectangles() -> crate::test_utils::EditorTestUtils {
		use crate::test_utils::test_prelude::*;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		for x in [0., 150., 300.] {
			editor.drag_tool(ToolType::Rectangle, x, 0., x + 100., 100., ModifierKeys::empty()).await;
			editor.handle_message(DocumentMessage::StartTransaction).await;
			editor.handle_message(DocumentMessage::EndTransaction).await;
			editor.active_document_mut().retire_storage_interaction();
		}
		editor
	}

	fn rows(editor: &crate::test_utils::EditorTestUtils) -> Vec<HistoryRow> {
		let document = editor.active_document();
		let gdd = document.storage().expect("every document has a working copy");
		panel_state(gdd, document.declarations(), None, PAGE, &BTreeSet::new(), None).rows
	}

	fn layer_count(editor: &crate::test_utils::EditorTestUtils) -> usize {
		editor.active_document().metadata().all_layers().count()
	}

	#[tokio::test]
	async fn removing_a_step_is_undone_and_redone_like_any_other() {
		let mut editor = three_rectangles().await;
		let second = rows(&editor)[1].id.clone();
		editor.handle_message(HistoryMessage::RemoveStep { id: second }).await;
		assert_eq!(layer_count(&editor), 2);

		editor.handle_message(DocumentMessage::DocumentHistoryBackward).await;
		assert_eq!(layer_count(&editor), 3, "undo brings the removed rectangle back");
		editor.handle_message(DocumentMessage::DocumentHistoryForward).await;
		assert_eq!(layer_count(&editor), 2, "redo removes it again");
		editor.handle_message(DocumentMessage::DocumentHistoryBackward).await;
		assert_eq!(layer_count(&editor), 3, "and undo takes the removal back again");
	}

	#[tokio::test]
	async fn previewing_a_step_shows_it_until_the_preview_ends_without_changing_history() {
		let mut editor = three_rectangles().await;
		let before = rows(&editor);
		let first = before[2].id.clone();

		editor.handle_message(HistoryMessage::Preview { id: Some(first.clone()) }).await;
		assert_eq!(layer_count(&editor), 1, "the first rectangle's step shows one rectangle");
		assert_eq!(editor.active_document().previewed_history_step(), parse_rev(&first));

		editor.handle_message(HistoryMessage::Preview { id: None }).await;
		assert_eq!(layer_count(&editor), 3);
		assert_eq!(editor.active_document().previewed_history_step(), None);
		assert_eq!(rows(&editor), before, "previewing leaves history as it was");
	}

	#[tokio::test]
	async fn editing_while_previewing_continues_from_the_previewed_step() {
		use crate::test_utils::test_prelude::*;

		let mut editor = three_rectangles().await;
		let first = rows(&editor)[2].id.clone();

		editor.handle_message(HistoryMessage::Preview { id: Some(first.clone()) }).await;
		editor.drag_tool(ToolType::Ellipse, 0., 150., 100., 250., ModifierKeys::empty()).await;
		editor.active_document_mut().retire_storage_interaction();

		assert_eq!(editor.active_document().previewed_history_step(), None);
		assert_eq!(layer_count(&editor), 2, "the ellipse is drawn next to the first rectangle only");
		let rows = rows(&editor);
		assert_eq!(rows[0].label, "Drew an Ellipse");
		let previewed = rows.iter().find(|row| row.id == first).expect("the previewed step is on the line");
		assert_eq!(previewed.branches.len(), 1, "the steps after it stay as a branch");
		let violations = editor.active_document().network_interface.validate_invariants();
		assert!(violations.is_empty(), "{violations:#?}");
	}

	#[tokio::test]
	async fn undoing_a_step_not_yet_in_history_keeps_it_listed_as_undone() {
		use crate::test_utils::test_prelude::*;

		let mut editor = three_rectangles().await;
		editor.drag_tool(ToolType::Ellipse, 0., 150., 100., 250., ModifierKeys::empty()).await;
		editor.handle_message(DocumentMessage::DocumentHistoryBackward).await;

		let rows = rows(&editor);
		assert_eq!(rows[0].label, "Drew an Ellipse");
		assert!(rows[0].undone, "the ellipse stays on the panel, undone");
		assert!(rows[1].head);
	}

	#[tokio::test]
	async fn removing_a_step_and_then_an_earlier_one_reopens() {
		use crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::build_interface_from_storage;

		let mut editor = three_rectangles().await;
		let second = rows(&editor)[1].id.clone();
		editor.handle_message(HistoryMessage::RemoveStep { id: second }).await;
		// The first rectangle's step brought the layer subgraph the third one is built from
		let first = rows(&editor).into_iter().filter(|row| row.label.contains("Rectangle")).last().expect("the first rectangle").id;
		editor.handle_message(HistoryMessage::RemoveStep { id: first }).await;
		assert_eq!(layer_count(&editor), 1);
		let violations = editor.active_document().network_interface.validate_invariants();
		assert!(violations.is_empty(), "{violations:#?}");

		let document = editor.active_document();
		let gdd = document.storage().expect("every document has a working copy").clone();
		let registry = gdd.registry().clone();
		let (working, layout) = gdd.into_storage();
		let reopened = document_format::GddV1::open_in(working, layout).await.expect("reopen");
		assert_eq!(reopened.registry(), &registry);
		let (network, nodes, networks) = reopened.registry().to_runtime_with_full_metadata(document.declarations(), reopened.blobs()).expect("convert");
		let interface = build_interface_from_storage(network, nodes, networks).expect("rebuild");
		let violations = interface.validate_invariants();
		assert!(violations.is_empty(), "{violations:#?}");
	}
}
