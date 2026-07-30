//! TreeEnsembleClassifier extraction logic.
//!
//! Converts ONNX TreeEnsembleClassifier operators into the internal
//! DecisionTree representation used by zkml-common.

use super::error::OnnxImportError;
use super::proto::NodeProto;
use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::{DecisionTree, TreeNode};

/// Extract a DecisionTree from a TreeEnsembleClassifier node.
///
/// # Arguments
///
/// * `node` - The TreeEnsembleClassifier node
/// * `num_features` - Number of input features from the model input shape
///
/// # Errors
///
/// - Returns an error if the operator is not a single tree (ensembles are rejected)
/// - Returns an error if node modes are unsupported (only BRANCH_LEQ is supported)
/// - Returns an error if structural validation fails (index bounds, cycles, leaf reachability)
pub fn extract_tree(
    node: &NodeProto,
    num_features: usize,
) -> Result<DecisionTree, OnnxImportError> {
    // Reject ensembles (only single trees supported for now)
    let num_trees = get_int_attribute(node, "n_trees").unwrap_or(1);
    if num_trees > 1 {
        return Err(OnnxImportError::MalformedModel(format!(
            "Multi-tree ensembles are not yet supported (found {num_trees} trees)"
        )));
    }

    // Extract tree structure attributes
    let nodes_featureids = get_ints_attribute(node, "nodes_featureids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing nodes_featureids".into()))?;
    let nodes_values = get_floats_attribute(node, "nodes_values")?;
    let nodes_modes = get_strings_attribute(node, "nodes_modes")?;
    let nodes_truenodeids = get_ints_attribute(node, "nodes_truenodeids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing nodes_truenodeids".into()))?;
    let nodes_falsenodeids = get_ints_attribute(node, "nodes_falsenodeids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing nodes_falsenodeids".into()))?;
    let _class_ids = get_ints_attribute(node, "class_ids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing class_ids".into()))?;
    let _class_weights = get_floats_attribute(node, "class_weights")?;

    // Validate attribute lengths are consistent
    let num_nodes = nodes_featureids.len();
    if nodes_values.len() != num_nodes
        || nodes_modes.len() != num_nodes
        || nodes_truenodeids.len() != num_nodes
        || nodes_falsenodeids.len() != num_nodes
    {
        return Err(OnnxImportError::MalformedModel(
            "inconsistent tree attribute lengths".into(),
        ));
    }

    // Build the flat node vector
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        let mode = &nodes_modes[i];
        match mode.as_str() {
            "BRANCH_LEQ" => {
                // Split node
                let feature_index = nodes_featureids[i] as usize;
                if feature_index >= num_features {
                    return Err(OnnxImportError::MalformedModel(format!(
                        "node {i}: feature index {feature_index} out of range (num_features={num_features})"
                    )));
                }
                let threshold = FixedPoint::quantize(nodes_values[i]);
                let left = nodes_truenodeids[i] as usize;
                let right = nodes_falsenodeids[i] as usize;
                nodes.push(TreeNode::Split {
                    feature_index,
                    threshold,
                    left,
                    right,
                });
            }
            "LEAF" => {
                // Leaf node - use nodes_values for the value
                // In skl2onnx, leaf values are stored in nodes_values
                let value = FixedPoint::quantize(nodes_values[i]);
                nodes.push(TreeNode::Leaf { value });
            }
            _ => {
                return Err(OnnxImportError::MalformedModel(format!(
                    "unsupported node mode '{mode}' (only BRANCH_LEQ and LEAF are supported)"
                )));
            }
        }
    }

    let tree = DecisionTree {
        nodes,
        num_features,
    };

    // Structural validation
    validate_tree_structure(&tree)?;

    Ok(tree)
}

/// Get an integer attribute value by name.
fn get_int_attribute(node: &NodeProto, name: &str) -> Option<i64> {
    node.attribute
        .iter()
        .find(|attr| attr.name == name)
        .and_then(|attr| attr.ints.first().copied())
}

/// Get a vector of integer attribute values by name.
fn get_ints_attribute(node: &NodeProto, name: &str) -> Option<Vec<i64>> {
    node.attribute
        .iter()
        .find(|attr| attr.name == name)
        .map(|attr| attr.ints.clone())
}

/// Get a vector of float attribute values by name.
fn get_floats_attribute(node: &NodeProto, name: &str) -> Result<Vec<f64>, OnnxImportError> {
    node.attribute
        .iter()
        .find(|attr| attr.name == name)
        .map(|attr| attr.floats.clone())
        .ok_or_else(|| OnnxImportError::MalformedModel(format!("missing attribute '{name}'")))
}

/// Get a vector of string attribute values by name.
fn get_strings_attribute(node: &NodeProto, name: &str) -> Result<Vec<String>, OnnxImportError> {
    node.attribute
        .iter()
        .find(|attr| attr.name == name)
        .map(|attr| attr.strings.clone())
        .ok_or_else(|| OnnxImportError::MalformedModel(format!("missing attribute '{name}'")))
}

/// Validate tree structure: index bounds, no cycles, leaf reachability.
fn validate_tree_structure(tree: &DecisionTree) -> Result<(), OnnxImportError> {
    let num_nodes = tree.nodes.len();

    // Check all child indices are in bounds
    for (i, node) in tree.nodes.iter().enumerate() {
        if let TreeNode::Split { left, right, .. } = node {
            if *left >= num_nodes {
                return Err(OnnxImportError::MalformedModel(format!(
                    "node {i}: left child index {left} out of bounds (num_nodes={num_nodes})"
                )));
            }
            if *right >= num_nodes {
                return Err(OnnxImportError::MalformedModel(format!(
                    "node {i}: right child index {right} out of bounds (num_nodes={num_nodes})"
                )));
            }
        }
    }

    // Check for cycles using DFS
    let mut visited = vec![false; num_nodes];
    let mut stack = vec![0usize]; // Start from root

    while let Some(node_idx) = stack.pop() {
        if node_idx >= num_nodes {
            return Err(OnnxImportError::MalformedModel(format!(
                "invalid node index {node_idx} during traversal"
            )));
        }

        if visited[node_idx] {
            return Err(OnnxImportError::MalformedModel(format!(
                "cycle detected at node {node_idx}"
            )));
        }

        visited[node_idx] = true;

        match &tree.nodes[node_idx] {
            TreeNode::Split { left, right, .. } => {
                stack.push(*left);
                stack.push(*right);
            }
            TreeNode::Leaf { .. } => {
                // Leaf node - stop traversal
            }
        }
    }

    // Check that all nodes are reachable from root
    for (i, &was_visited) in visited.iter().enumerate() {
        if !was_visited {
            return Err(OnnxImportError::MalformedModel(format!(
                "node {i} is not reachable from root"
            )));
        }
    }

    // Check that every path reaches a leaf (no split-only paths)
    for (_i, node) in tree.nodes.iter().enumerate() {
        if let TreeNode::Split { left, right, .. } = node {
            if !matches!(tree.nodes[*left], TreeNode::Leaf { .. })
                && !matches!(tree.nodes[*right], TreeNode::Leaf { .. })
            {
                // This is OK - internal nodes can have split children
                // We just need to ensure the tree terminates somewhere
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onnx::proto::{AttributeProto, NodeProto};

    fn make_tree_node() -> NodeProto {
        NodeProto {
            name: "tree".into(),
            op_type: "TreeEnsembleClassifier".into(),
            domain: "ai.onnx.ml".into(),
            input: vec!["X".into()],
            output: vec!["Y".into()],
            attribute: vec![
                AttributeProto {
                    name: "n_trees".into(),
                    ints: vec![1],
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_featureids".into(),
                    ints: vec![0, 0, 0], // 3 nodes
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_values".into(),
                    floats: vec![0.5, 0.0, 1.0], // 3 nodes
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_modes".into(),
                    strings: vec!["BRANCH_LEQ".into(), "LEAF".into(), "LEAF".into()],
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_truenodeids".into(),
                    ints: vec![1, 0, 0],
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_falsenodeids".into(),
                    ints: vec![2, 0, 0],
                    ..Default::default()
                },
                AttributeProto {
                    name: "class_ids".into(),
                    ints: vec![0, 0, 0],
                    ..Default::default()
                },
                AttributeProto {
                    name: "class_weights".into(),
                    floats: vec![0.0, 1.0],
                    ..Default::default()
                },
            ],
        }
    }

    #[test]
    fn test_simple_tree_extraction() {
        let node = make_tree_node();
        let tree = extract_tree(&node, 1).unwrap();
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.num_features, 1);
    }

    #[test]
    fn test_reject_ensemble() {
        let mut node = make_tree_node();
        // Change n_trees to 2
        if let Some(attr) = node.attribute.iter_mut().find(|a| a.name == "n_trees") {
            attr.ints[0] = 2;
        }
        let err = extract_tree(&node, 1).unwrap_err();
        assert!(matches!(
            err,
            OnnxImportError::MalformedModel(msg) if msg.contains("Multi-tree ensembles")
        ));
    }

    #[test]
    fn test_reject_unsupported_mode() {
        let mut node = make_tree_node();
        // Change mode to unsupported value
        if let Some(attr) = node.attribute.iter_mut().find(|a| a.name == "nodes_modes") {
            attr.strings[0] = "BRANCH_LT".into();
        }
        let err = extract_tree(&node, 1).unwrap_err();
        assert!(matches!(
            err,
            OnnxImportError::MalformedModel(msg) if msg.contains("unsupported node mode")
        ));
    }

    #[test]
    fn test_detect_out_of_bounds_child() {
        let mut node = make_tree_node();
        // Set left child to invalid index
        if let Some(attr) = node
            .attribute
            .iter_mut()
            .find(|a| a.name == "nodes_truenodeids")
        {
            attr.ints[0] = 99;
        }
        let err = extract_tree(&node, 1).unwrap_err();
        assert!(matches!(
            err,
            OnnxImportError::MalformedModel(msg) if msg.contains("out of bounds")
        ));
    }
}
