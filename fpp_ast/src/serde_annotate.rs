//! Serde glue for serializing an `#[ast]` nodes.

use fpp_core::{Annotated, Node};
use serde::Serializer;
use serde::ser::SerializeStruct;

pub(crate) fn serialize<S: Serializer>(node: &Node, serializer: S) -> Result<S::Ok, S::Error> {
    let mut s = serializer.serialize_struct("Annotations", 2)?;
    s.serialize_field("pre", &node.pre_annotation())?;
    s.serialize_field("post", &node.post_annotation())?;
    s.end()
}
