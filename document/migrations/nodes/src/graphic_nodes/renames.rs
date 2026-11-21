//! Nodes that only moved to a new identifier, or changed version with stored nodes staying as they are.

use migration_core::rename;

// Version 2 only adds `InjectIndex` to the context bounds, so stored nodes keep their layout
rename!("graphic_nodes::graphic::MapNode"@1 => @2);
rename!("graphic_nodes::graphic::IterateNode"@1 => @2);
rename!("graphic_nodes::graphic::WriteAttributeNode"@1 => @2);
