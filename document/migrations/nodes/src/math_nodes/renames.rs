//! Nodes that only moved to a new identifier, or changed version with stored nodes staying as they are.

use migration_core::rename;

rename!("math_nodes::MathFxNode"@1 => "math_nodes::MathFunctionNode"@2);
rename!("math_nodes::MathFNode"@1 => "math_nodes::MathFormulaNode"@2);
