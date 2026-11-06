//! Every migration together, since a duplicate or a cycle would fail every document with a pending migration.

use migration_core::registered_migrations;
use migration_nodes as _;
use migration_runner::order_migrations;

#[test]
fn every_migration_registers_and_orders() {
	let migrations = registered_migrations();
	assert!(!migrations.is_empty(), "the linker dropped the migrations' registrations");
	order_migrations(&migrations).unwrap();
}
