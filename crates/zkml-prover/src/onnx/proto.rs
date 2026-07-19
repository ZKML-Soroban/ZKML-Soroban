//! Minimal ONNX protobuf schema used by the importer foundation.
//!
//! Field numbers match the official ONNX `onnx.proto` definitions so that
//! real `.onnx` files decode correctly for the messages we care about
//! (model metadata, opset imports, graph nodes).
//!
//! Only the surface required for parse + validation is modelled. Attribute
//! and tensor payloads needed for parameter extraction will be added with
//! issues #5 / #6.
//!
//! Schema reference: ONNX IR (opset 17+ era), `ModelProto` / `GraphProto` /
//! `NodeProto` / `OperatorSetIdProto`.

use prost::Message;

/// Top-level ONNX model container (`ModelProto`).
#[derive(Clone, PartialEq, Message)]
pub struct ModelProto {
    /// IR version of the model encoding.
    #[prost(int64, tag = "1")]
    pub ir_version: i64,
    /// Human-readable producer name (optional).
    #[prost(string, tag = "2")]
    pub producer_name: String,
    /// Producer version string (optional).
    #[prost(string, tag = "3")]
    pub producer_version: String,
    /// Domain name for the model (optional).
    #[prost(string, tag = "4")]
    pub domain: String,
    /// Model version number (optional).
    #[prost(int64, tag = "5")]
    pub model_version: i64,
    /// Free-form documentation string.
    #[prost(string, tag = "6")]
    pub doc_string: String,
    /// Computation graph.
    #[prost(message, optional, tag = "7")]
    pub graph: Option<GraphProto>,
    /// Operator set imports declaring domain + version pairs.
    #[prost(message, repeated, tag = "8")]
    pub opset_import: Vec<OperatorSetIdProto>,
}

/// Operator set identifier (`OperatorSetIdProto`).
#[derive(Clone, PartialEq, Message)]
pub struct OperatorSetIdProto {
    /// Domain name. Empty string means the default ONNX domain (`ai.onnx`).
    #[prost(string, tag = "1")]
    pub domain: String,
    /// Opset version for this domain.
    #[prost(int64, tag = "2")]
    pub version: i64,
}

/// Computation graph (`GraphProto`).
#[derive(Clone, PartialEq, Message)]
pub struct GraphProto {
    /// Ordered list of operator nodes.
    #[prost(message, repeated, tag = "1")]
    pub node: Vec<NodeProto>,
    /// Graph name.
    #[prost(string, tag = "2")]
    pub name: String,
}

/// Single operator node (`NodeProto`).
#[derive(Clone, PartialEq, Message)]
pub struct NodeProto {
    /// Input tensor names.
    #[prost(string, repeated, tag = "1")]
    pub input: Vec<String>,
    /// Output tensor names.
    #[prost(string, repeated, tag = "2")]
    pub output: Vec<String>,
    /// Optional node name.
    #[prost(string, tag = "3")]
    pub name: String,
    /// Operator type (e.g. `MatMul`, `TreeEnsembleClassifier`).
    #[prost(string, tag = "4")]
    pub op_type: String,
    /// Operator domain. Empty means default ONNX domain.
    #[prost(string, tag = "7")]
    pub domain: String,
}
