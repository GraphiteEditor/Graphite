use migration_core::{FormatMigration, HistoryPolicy, MigrationError, Payload, PayloadCodec};
use migration_runner::{DocumentPayloads, HistoryOutcome, RunnerError, run_format_migrations};

/// A v1 → v2 format step that renames a top-level registry key, exercising the frozen-shape
/// decode/encode round trip.
struct RenameKey;

impl FormatMigration for RenameKey {
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

#[test]
fn format_tier_chains_and_bumps_version() {
	let mut payloads = DocumentPayloads {
		format_version: 1,
		registry: Some(Payload::encode(&serde_json::json!({ "old_key": 42 }), PayloadCodec::Json).unwrap()),
		history: vec![Payload::new(vec![1, 2, 3], PayloadCodec::Postcard)],
		hot_log: Vec::new(),
		session: None,
	};

	let outcome = run_format_migrations(&[&RenameKey], &mut payloads, 2).unwrap();

	assert_eq!(payloads.format_version, 2);
	assert_eq!(outcome, HistoryOutcome::Truncated);
	assert!(payloads.history.is_empty());
	let registry: serde_json::Value = payloads.registry.unwrap().decode().unwrap();
	assert_eq!(registry, serde_json::json!({ "new_key": 42 }));
}

#[test]
fn format_tier_errors_on_missing_step() {
	let mut payloads = DocumentPayloads {
		format_version: 0,
		registry: Some(Payload::encode(&serde_json::json!({}), PayloadCodec::Json).unwrap()),
		history: Vec::new(),
		hot_log: Vec::new(),
		session: None,
	};

	let error = run_format_migrations(&[&RenameKey], &mut payloads, 2).unwrap_err();
	assert!(matches!(error, RunnerError::MissingFormatStep { at_version: 0 }));
}

#[test]
fn truncating_a_document_without_a_registry_fails() {
	let mut payloads = DocumentPayloads {
		format_version: 1,
		registry: None,
		history: vec![Payload::new(vec![1, 2, 3], PayloadCodec::Postcard)],
		hot_log: Vec::new(),
		session: None,
	};

	let error = run_format_migrations(&[&RenameKey], &mut payloads, 2).unwrap_err();

	assert!(matches!(error, RunnerError::TruncatesOnlyCopy { at_version: 1 }), "{error}");
	assert_eq!(payloads.history.len(), 1);
}
