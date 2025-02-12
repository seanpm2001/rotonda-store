//! A treebitmap based IP Prefix Store

//! IP prefixes storage and retrieval data structures for IPv4 and IPv6 prefixes.
//! This crate contains structures for both single and multi-threaded contexts, as
//! well as async contexts.
//!
//! The underlying tree structure is based on the tree bitmap as outlined in
//! [this paper](https://www.cs.cornell.edu/courses/cs419/2005sp/tree-bitmap.pdf).
//!
//! Part of the Rotonda modular BGP engine.

//! Read more about the data-structure in this [blog post](https://blog.nlnetlabs.nl/donkeys-mules-horses/).
mod af;
mod local_array;
mod local_vec;
mod node_id;
mod prefix_record;
mod stride;
mod synth_int;

#[macro_use]
mod macros;

mod rotonda_store;

// Public Interfaces

pub mod prelude;
/// Statistics for the two trees (IPv4 and IPv6).
pub mod stats;

/// Some simple metadata implementations
pub mod meta_examples;

/// The publicly available devices
pub use crate::rotonda_store::*;

// re-exports
pub use inetnum::addr;
pub use crossbeam_epoch::{self as epoch, Guard};
