use graph_storage::{Attributes, AttributesRead, AttributesWrite, Implementation, Network, Node, PeerId, ROOT_NETWORK, Registry, ResourceId, Session, TimeStamp};
use migration_core::{
	ContentMigration, DeclarationInfo, FormatMigration, HistoryPolicy, MigrationContext, MigrationError, MigrationHost, MigrationId, MigrationSet, Payload, PayloadCodec, Selector, Target,
};
use migration_runner::{DocumentPayloads, HistoryOutcome, MigrationRunner, RunnerError};

struct NoHost;

impl MigrationHost for NoHost {
	fn declaration_info(&self, _id: ResourceId) -> Option<DeclarationInfo> {
		None
	}
	fn declaration(&self, _id: ResourceId) -> Option<serde_json::Value> {
		None
	}
	fn resolve_definition(&mut self, _identifier: &str) -> Option<Node> {
		None
	}
}

/// A v1 → v2 format step that renames a top-level registry key, exercising the frozen-shape
/// decode/encode round trip.
struct RenameKey;

impl FormatMigration for RenameKey {
	fn id(&self) -> MigrationId {
		MigrationId("test-rename-key")
	}
	fn migrates_from(&self) -> u32 {
		1
	}
	fn migrate_registry(&self, registry: &Payload) -> Result<Payload, MigrationError> {
		let mut value: serde_json::Value = registry.decode()?;
		if let Some(object) = value.as_object_mut()
			&& let Some(old) = object.remove("old_key")
		{
			object.insert("new_key".to_string(), old);
		}
		registry.encode_as(&value)
	}
	fn history_policy(&self) -> HistoryPolicy {
		HistoryPolicy::Truncate
	}
}

fn format_set() -> MigrationSet {
	MigrationSet {
		format: vec![Box::new(RenameKey)],
		..Default::default()
	}
}

#[test]
fn format_tier_chains_and_bumps_version() {
	let runner = MigrationRunner::new(vec![format_set()]);
	let mut payloads = DocumentPayloads {
		format_version: 1,
		registry: Payload::encode(&serde_json::json!({ "old_key": 42 }), PayloadCodec::Json).unwrap(),
		history: vec![Payload::new(vec![1, 2, 3], PayloadCodec::MessagePack)],
		session: None,
	};

	let outcome = runner.run_format_migrations(&mut payloads, 2).unwrap();

	assert_eq!(payloads.format_version, 2);
	assert_eq!(outcome, HistoryOutcome::Truncated);
	assert!(payloads.history.is_empty());
	let registry: serde_json::Value = payloads.registry.decode().unwrap();
	assert_eq!(registry, serde_json::json!({ "new_key": 42 }));
}

#[test]
fn format_tier_errors_on_missing_step() {
	let runner = MigrationRunner::new(vec![format_set()]);
	let mut payloads = DocumentPayloads {
		format_version: 0,
		registry: Payload::encode(&serde_json::json!({}), PayloadCodec::Json).unwrap(),
		history: Vec::new(),
		session: None,
	};

	let error = runner.run_format_migrations(&mut payloads, 2).unwrap_err();
	assert!(matches!(error, RunnerError::MissingFormatStep { at_version: 0 }));
}

/// Flags every "Brush"-referenced node with a marker attribute, exercising the delta-expressed
/// clone-mutate-diff-commit path.
struct FlagBrushNodes;

impl ContentMigration for FlagBrushNodes {
	fn id(&self) -> MigrationId {
		MigrationId("test-flag-brush")
	}
	fn selector(&self) -> Selector {
		Selector::Reference("Brush")
	}
	fn migrate(&self, target: Target, registry: &mut Registry, _context: &mut dyn MigrationContext) -> Result<(), MigrationError> {
		let Target::Node(node_id) = target else { return Ok(()) };
		let node = registry.node_instances.get_mut(&node_id).ok_or(MigrationError::Invariant("matched node missing".to_string()))?;
		node.attributes_mut().set("migrated", serde_json::Value::Bool(true), TimeStamp::default());
		Ok(())
	}
}

fn seeded_session() -> (Session, graph_storage::NodeId) {
	let mut session = Session::with_peer(PeerId(7));
	let node_id = graph_storage::NodeId(99);

	let mut registry = Registry::default();
	registry.networks.insert(ROOT_NETWORK, Network::default());
	let mut attributes = Attributes::new();
	attributes.set(graph_storage::attr::node::ui::REFERENCE, serde_json::Value::String("Brush".to_string()), TimeStamp::default());
	registry
		.node_instances
		.insert(node_id, Node::new(Implementation::ProtoNode(ResourceId::new()), Vec::new(), attributes, ROOT_NETWORK));

	session.stage_registry_replace(&registry).unwrap();
	session.retire_all().unwrap();

	(session, node_id)
}

#[test]
fn content_tier_commits_migration_as_history_deltas() {
	let runner = MigrationRunner::new(vec![MigrationSet {
		content: vec![Box::new(FlagBrushNodes)],
		..Default::default()
	}]);

	let (mut session, node_id) = seeded_session();
	let history_before = session.history().count();

	let revs = runner.run_content_migrations(&mut session, &mut NoHost).unwrap();

	assert!(!revs.is_empty());
	assert!(session.history().count() > history_before);
	let node = &session.registry().node_instances[&node_id];
	assert_eq!(node.attributes().get_typed::<bool>("migrated"), Some(true));

	// Idempotency: a second run matches the same selector but changes nothing, so no new deltas
	let history_after_first = session.history().count();
	let revs = runner.run_content_migrations(&mut session, &mut NoHost).unwrap();
	assert!(revs.is_empty());
	assert_eq!(session.history().count(), history_after_first);
}
