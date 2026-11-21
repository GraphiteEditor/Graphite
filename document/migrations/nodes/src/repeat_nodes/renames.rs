//! Nodes that only moved to a new identifier, or changed version with stored nodes staying as they are.

use migration_core::rename;

// Version 2 only adds `InjectIndex` to the context bounds, so stored nodes keep their layout
rename!("repeat_nodes::repeat_nodes::RepeatNode"@1 => @2);
rename!("repeat_nodes::repeat_nodes::RepeatArrayNode"@1 => @2);
rename!("repeat_nodes::repeat_nodes::RepeatRadialNode"@1 => @2);
rename!("repeat_nodes::repeat_nodes::RepeatOnPointsNode"@1 => @2);
