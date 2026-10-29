use crate::NodeMigration;
use std::sync::{Mutex, PoisonError};

static MIGRATIONS: Mutex<Vec<&'static dyn NodeMigration>> = Mutex::new(Vec::new());

/// Add a node migration to the registry. Called at startup by what `#[migration]` and `rename!` generate.
pub fn register_migration(migration: &'static dyn NodeMigration) {
	MIGRATIONS.lock().unwrap_or_else(PoisonError::into_inner).push(migration);
}

/// Every registered node migration, in registration order.
pub fn registered_migrations() -> Vec<&'static dyn NodeMigration> {
	MIGRATIONS.lock().unwrap_or_else(PoisonError::into_inner).clone()
}
