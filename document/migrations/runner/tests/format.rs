use migration_core::{FormatMigration, HistoryPolicy, MigrationError, MigrationId, MigrationSet, Payload, PayloadCodec};
use migration_runner::{DocumentPayloads, HistoryOutcome, MigrationRunner, RunnerError};

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
		history: vec![Payload::new(vec![1, 2, 3], PayloadCodec::Postcard)],
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
