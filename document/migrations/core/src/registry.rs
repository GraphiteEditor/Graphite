use crate::FormatMigration;
#[cfg(feature = "typed")]
use crate::NodeMigration;
use std::sync::{Mutex, PoisonError};

#[cfg(feature = "typed")]
static MIGRATIONS: Mutex<Vec<&'static dyn NodeMigration>> = Mutex::new(Vec::new());
static FORMAT_MIGRATIONS: Mutex<Vec<&'static dyn FormatMigration>> = Mutex::new(Vec::new());

/// Add a node migration to the registry. Called at startup by what `#[migration]` and `rename!` generate.
#[cfg(feature = "typed")]
pub fn register_migration(migration: &'static dyn NodeMigration) {
	MIGRATIONS.lock().unwrap_or_else(PoisonError::into_inner).push(migration);
}

/// Every registered node migration, in registration order.
#[cfg(feature = "typed")]
pub fn registered_migrations() -> Vec<&'static dyn NodeMigration> {
	MIGRATIONS.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Add a format step to the registry. Called at startup by what `format_migration!` generates.
pub fn register_format_migration(migration: &'static dyn FormatMigration) {
	FORMAT_MIGRATIONS.lock().unwrap_or_else(PoisonError::into_inner).push(migration);
}

/// Every registered format step, in registration order.
pub fn registered_format_migrations() -> Vec<&'static dyn FormatMigration> {
	FORMAT_MIGRATIONS.lock().unwrap_or_else(PoisonError::into_inner).clone()
}
