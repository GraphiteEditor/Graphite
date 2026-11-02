//! Node migrations that ask the compiler about a document, and [`CompilerHost`], which the editor and CLI offer it through.
//! Unlike `migration-nodes`, these link the node crates and depend on the build they run in, so a document's peers can't all reach the same result.

mod compiler;

pub use compiler::CompilerHost;
