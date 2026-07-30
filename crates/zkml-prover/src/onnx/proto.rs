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
    /// Input tensors.
    #[prost(message, repeated, tag = "3")]
    pub input: Vec<ValueInfoProto>,
    /// Output tensors.
    #[prost(message, repeated, tag = "4")]
    pub output: Vec<ValueInfoProto>,
}

/// Tensor shape information (`ValueInfoProto`).
#[derive(Clone, PartialEq, Message)]
pub struct ValueInfoProto {
    /// Tensor name.
    #[prost(string, tag = "1")]
    pub name: String,
    /// Type information including shape.
    #[prost(message, optional, tag = "2")]
    pub r#type: Option<TypeProto>,
}

/// Type information (`TypeProto`).
#[derive(Clone, PartialEq, Message)]
pub struct TypeProto {
    /// Tensor type and shape.
    #[prost(message, optional, tag = "1")]
    pub tensor: Option<TensorTypeProto>,
}

/// Tensor type and shape (`TensorTypeProto`).
#[derive(Clone, PartialEq, Message)]
pub struct TensorTypeProto {
    /// Element type (not used for shape extraction).
    #[prost(enumeration = "TensorDataType", tag = "1")]
    pub elem_type: i32,
    /// Shape information.
    #[prost(message, optional, tag = "2")]
    pub shape: Option<TensorShapeProto>,
}

/// Tensor shape (`TensorShapeProto`).
#[derive(Clone, PartialEq, Message)]
pub struct TensorShapeProto {
    /// Dimensions.
    #[prost(message, repeated, tag = "1")]
    pub dim: Vec<TensorShapeProtoDimension>,
}

/// Single dimension in a tensor shape.
#[derive(Clone, PartialEq, Message)]
pub struct TensorShapeProtoDimension {
    /// Dimension value (if known).
    #[prost(int64, tag = "1")]
    pub dim_value: i64,
    /// Dimension parameter name (if symbolic).
    #[prost(string, tag = "2")]
    pub dim_param: String,
}

/// Tensor data types (enumeration).
#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
pub enum TensorDataType {
    Undefined = 0,
    Float = 1,
    Uint8 = 2,
    Int8 = 3,
    Uint16 = 4,
    Int16 = 5,
    Int32 = 6,
    Int64 = 7,
    String = 8,
    Bool = 9,
    Float16 = 10,
    Double = 11,
    Uint32 = 12,
    Uint64 = 13,
    Complex64 = 14,
    Complex128 = 15,
    Bfloat16 = 16,
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
    /// Operator attributes.
    #[prost(message, repeated, tag = "5")]
    pub attribute: Vec<AttributeProto>,
}

/// Operator attribute (`AttributeProto`).
///
/// Used to extract TreeEnsembleClassifier parameters like node thresholds,
/// feature indices, and class weights.
#[derive(Clone, PartialEq, Message)]
pub struct AttributeProto {
    /// Attribute name.
    #[prost(string, tag = "1")]
    pub name: String,
    /// Float values (for thresholds, leaf values, class weights).
    #[prost(double, repeated, tag = "5")]
    pub floats: Vec<f64>,
    /// Integer values (for feature indices, node IDs, class IDs).
    #[prost(int64, repeated, tag = "6")]
    pub ints: Vec<i64>,
    /// String values (for node modes like "BRANCH_LEQ").
    #[prost(string, repeated, tag = "8")]
    pub strings: Vec<String>,
}
