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
/// Leaf values: a leaf with one class weight keeps that weight as its value.
/// A leaf with one weight per class, which is what skl2onnx writes, takes the
/// predicted class as its value: the class with the largest weight, ties going
/// to the lowest class id, as `argmax` does elsewhere in the engine. The class
/// id is mapped through `classlabels_int64s` when the model carries it, so the
/// value is the label the model predicts. Text labels cannot be a fixed-point
/// value and are refused for such leaves.
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
    // The number of trees is the count of distinct values in nodes_treeids
    let nodes_treeids = get_ints_attribute(node, "nodes_treeids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing nodes_treeids".into()))?;
    let num_trees: std::collections::HashSet<_> = nodes_treeids.iter().cloned().collect();
    if num_trees.len() > 1 {
        return Err(OnnxImportError::MalformedModel(format!(
            "Multi-tree ensembles are not yet supported (found {} trees)",
            num_trees.len()
        )));
    }

    // Extract tree structure attributes
    let nodes_nodeids = get_ints_attribute(node, "nodes_nodeids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing nodes_nodeids".into()))?;
    let nodes_featureids = get_ints_attribute(node, "nodes_featureids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing nodes_featureids".into()))?;
    let nodes_values = get_floats_attribute(node, "nodes_values")?;
    let nodes_modes = get_strings_attribute(node, "nodes_modes")?;
    let nodes_truenodeids = get_ints_attribute(node, "nodes_truenodeids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing nodes_truenodeids".into()))?;
    let nodes_falsenodeids = get_ints_attribute(node, "nodes_falsenodeids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing nodes_falsenodeids".into()))?;

    // Extract class attributes for leaf values
    let class_ids = get_ints_attribute(node, "class_ids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing class_ids".into()))?;
    let class_weights = get_floats_attribute(node, "class_weights")?;
    let class_nodeids = get_ints_attribute(node, "class_nodeids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing class_nodeids".into()))?;
    let class_treeids = get_ints_attribute(node, "class_treeids")
        .ok_or_else(|| OnnxImportError::MalformedModel("missing class_treeids".into()))?;

    // Validate attribute lengths are consistent
    let num_nodes = nodes_featureids.len();
    if nodes_nodeids.len() != num_nodes
        || nodes_values.len() != num_nodes
        || nodes_modes.len() != num_nodes
        || nodes_truenodeids.len() != num_nodes
        || nodes_falsenodeids.len() != num_nodes
    {
        return Err(OnnxImportError::MalformedModel(
            "inconsistent tree attribute lengths".into(),
        ));
    }

    // Build a map from node IDs to array indices
    let mut node_id_to_index = std::collections::HashMap::new();
    for (i, &node_id) in nodes_nodeids.iter().enumerate() {
        node_id_to_index.insert(node_id, i);
    }

    let leaf_values = leaf_values(
        node,
        &class_ids,
        &class_weights,
        &class_nodeids,
        &class_treeids,
    )?;

    // Build the flat node vector
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        let node_id = nodes_nodeids[i];
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
                let threshold = FixedPoint::try_quantize(nodes_values[i]).map_err(|reason| {
                    OnnxImportError::MalformedModel(format!("node {i} threshold: {reason}"))
                })?;

                // Map child node IDs to array indices
                let true_child_id = nodes_truenodeids[i];
                let false_child_id = nodes_falsenodeids[i];
                let left = *node_id_to_index.get(&true_child_id).ok_or_else(|| {
                    OnnxImportError::MalformedModel(format!(
                        "node {i}: true child node id {true_child_id} not found in nodes_nodeids"
                    ))
                })?;
                let right = *node_id_to_index.get(&false_child_id).ok_or_else(|| {
                    OnnxImportError::MalformedModel(format!(
                        "node {i}: false child node id {false_child_id} not found in nodes_nodeids"
                    ))
                })?;

                nodes.push(TreeNode::Split {
                    feature_index,
                    threshold,
                    left,
                    right,
                });
            }
            "LEAF" => {
                // Leaf node - use class_weights for the value
                let value = *leaf_values.get(&node_id).ok_or_else(|| {
                    OnnxImportError::MalformedModel(format!(
                        "node {i}: leaf node id {node_id} not found in class_nodeids"
                    ))
                })?;
                nodes.push(TreeNode::Leaf {
                    value: FixedPoint::try_quantize(value).map_err(|reason| {
                        OnnxImportError::MalformedModel(format!("leaf node {i}: {reason}"))
                    })?,
                });
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

/// The value of each leaf, keyed by leaf node id. See [`extract_tree`].
///
/// `class_nodeids[k]` names the leaf that `class_weights[k]` belongs to, for
/// class `class_ids[k]` of tree `class_treeids[k]`. The four lists are parallel,
/// so their lengths are checked before any of them is indexed.
fn leaf_values(
    node: &NodeProto,
    class_ids: &[i64],
    class_weights: &[f64],
    class_nodeids: &[i64],
    class_treeids: &[i64],
) -> Result<std::collections::HashMap<i64, f64>, OnnxImportError> {
    let n = class_nodeids.len();
    if class_ids.len() != n || class_weights.len() != n || class_treeids.len() != n {
        return Err(OnnxImportError::MalformedModel(
            "inconsistent class attribute lengths".into(),
        ));
    }

    let mut per_leaf: std::collections::HashMap<i64, Vec<(i64, f64)>> =
        std::collections::HashMap::new();
    for k in 0..n {
        if class_treeids[k] != 0 {
            return Err(OnnxImportError::MalformedModel(
                "multi-tree class weights not supported".into(),
            ));
        }
        per_leaf
            .entry(class_nodeids[k])
            .or_default()
            .push((class_ids[k], class_weights[k]));
    }

    let int_labels = get_ints_attribute(node, "classlabels_int64s").filter(|l| !l.is_empty());
    let text_labels = node
        .attribute
        .iter()
        .any(|attr| attr.name == "classlabels_strings" && !attr.strings.is_empty());

    let mut values = std::collections::HashMap::new();
    for (leaf, entries) in per_leaf {
        let value = match entries.as_slice() {
            [(_, weight)] => *weight,
            _ => {
                if entries.iter().any(|(_, weight)| !weight.is_finite()) {
                    return Err(OnnxImportError::MalformedModel(format!(
                        "leaf node id {leaf}: class weight is not finite"
                    )));
                }
                if text_labels {
                    return Err(OnnxImportError::MalformedModel(format!(
                        "leaf node id {leaf}: trees with text class labels are not supported; \
                         export the classifier with integer labels"
                    )));
                }
                // The predicted class: largest weight, lowest class id on a tie.
                let (class_id, _) = entries
                    .iter()
                    .copied()
                    .reduce(|best, next| {
                        if next.1 > best.1 || (next.1 == best.1 && next.0 < best.0) {
                            next
                        } else {
                            best
                        }
                    })
                    .expect("a leaf entry exists for every key");
                match &int_labels {
                    Some(labels) => {
                        let label = usize::try_from(class_id)
                            .ok()
                            .and_then(|i| labels.get(i))
                            .ok_or_else(|| {
                                OnnxImportError::MalformedModel(format!(
                                    "leaf node id {leaf}: class id {class_id} has no entry in \
                                     classlabels_int64s"
                                ))
                            })?;
                        *label as f64
                    }
                    None => class_id as f64,
                }
            }
        };
        values.insert(leaf, value);
    }
    Ok(values)
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
        .map(|attr| attr.floats.iter().map(|&x| x as f64).collect())
        .ok_or_else(|| OnnxImportError::MalformedModel(format!("missing attribute '{name}'")))
}

/// Get a vector of string attribute values by name.
///
/// Invalid UTF-8 is an error rather than a skipped entry, so a dropped mode
/// cannot quietly shorten `nodes_modes` below the length check above.
fn get_strings_attribute(node: &NodeProto, name: &str) -> Result<Vec<String>, OnnxImportError> {
    let attr = node
        .attribute
        .iter()
        .find(|attr| attr.name == name)
        .ok_or_else(|| OnnxImportError::MalformedModel(format!("missing attribute '{name}'")))?;

    attr.strings
        .iter()
        .map(|bytes| {
            String::from_utf8(bytes.clone()).map_err(|_| {
                OnnxImportError::MalformedModel(format!(
                    "attribute '{name}' holds a string that is not valid UTF-8"
                ))
            })
        })
        .collect()
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
    for node in tree.nodes.iter() {
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
                // Tree structure attributes
                AttributeProto {
                    name: "nodes_treeids".into(),
                    ints: vec![0, 0, 0], // All nodes in tree 0
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_nodeids".into(),
                    ints: vec![0, 1, 2], // Node IDs
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_featureids".into(),
                    ints: vec![0, 0, 0], // 3 nodes
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_values".into(),
                    floats: vec![0.5, 0.0, 0.0], // Thresholds (leaf values are 0 in ONNX)
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_modes".into(),
                    strings: vec!["BRANCH_LEQ".into(), "LEAF".into(), "LEAF".into()],
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_truenodeids".into(),
                    ints: vec![1, 0, 0], // Child node IDs (not indices)
                    ..Default::default()
                },
                AttributeProto {
                    name: "nodes_falsenodeids".into(),
                    ints: vec![2, 0, 0], // Child node IDs (not indices)
                    ..Default::default()
                },
                // Class attributes for leaf values
                AttributeProto {
                    name: "class_ids".into(),
                    ints: vec![0, 1], // Class indices
                    ..Default::default()
                },
                AttributeProto {
                    name: "class_weights".into(),
                    floats: vec![0.0, 1.0], // Actual leaf values
                    ..Default::default()
                },
                AttributeProto {
                    name: "class_nodeids".into(),
                    ints: vec![1, 2], // Which leaf nodes these weights belong to
                    ..Default::default()
                },
                AttributeProto {
                    name: "class_treeids".into(),
                    ints: vec![0, 0], // All in tree 0
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
        // Add a second tree by modifying nodes_treeids
        if let Some(attr) = node
            .attribute
            .iter_mut()
            .find(|a| a.name == "nodes_treeids")
        {
            attr.ints = vec![0, 0, 1]; // Node 2 is in tree 1
        }
        // Also need to add corresponding class_treeids
        if let Some(attr) = node
            .attribute
            .iter_mut()
            .find(|a| a.name == "class_treeids")
        {
            attr.ints = vec![0, 1]; // Second leaf in tree 1
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
        // Set left child to invalid node ID
        if let Some(attr) = node
            .attribute
            .iter_mut()
            .find(|a| a.name == "nodes_truenodeids")
        {
            attr.ints[0] = 99; // Node ID 99 doesn't exist
        }
        let err = extract_tree(&node, 1).unwrap_err();
        assert!(matches!(
            err,
            OnnxImportError::MalformedModel(msg) if msg.contains("not found in nodes_nodeids")
        ));
    }
}
