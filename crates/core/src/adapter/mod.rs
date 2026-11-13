//! The adapter: everything that knows Claude Code's undocumented file formats lives here
//! (docs/data-contract.md, section 1). Parsers tolerate unknown and missing fields and never panic.

pub mod bash;
pub mod paths;
pub mod records;
pub mod script;
pub mod text;
pub mod time;

pub use paths::{classify, normalise, ClaudePath};
