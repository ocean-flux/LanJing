//! Definition validation partitions sharing one analyzed graph.

mod control;
mod definition;
mod edges;
mod intents;

pub(in crate::compiler) use control::*;
pub(in crate::compiler) use definition::*;
pub(in crate::compiler) use edges::*;
pub(in crate::compiler) use intents::*;
