//! Nodes that only moved to a new identifier, or changed version with stored nodes staying as they are.

use migration_core::rename;

// Version 2 only adds `InjectIndex` to the context bounds, so stored nodes keep their layout
rename!("text_nodes::StringMapNode"@1 => @2);
