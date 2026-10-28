//! Writing the static bounding-volume trees of compressed and extern meshes: a tree's shape, and the bytes of its nodes.
//!
//! A tree is a pre-order array of nodes ([`crate::bvtree`] has the layout). A node's box is not stored as numbers but as
//! three nibble pairs that shrink the parent's box: on each axis the high nibble `h` takes `extent × h² / 226` from the
//! minimum side and the low nibble `l` takes `extent × l² / 226` from the maximum side. Encoding a node is choosing the
//! nibbles whose box is the smallest that still holds what lies under the node. [`encode_tree`] does that for a given
//! [`Shape`] and boxes of the leaves; [`balanced_shape`] makes a shape for boxes.
//!
//! **What the stock files show** (a census over every section tree of the Sekiro and Elden Ring map, object and asset
//! collision, 200,418 inner and 205,670 leaf nodes of the Sekiro objects alone): with the leaf boxes taken from the
//! decoded vertices grown by the shape's convex radius and the nodes' boxes taken from their stored parents, this rule
//! gives the stored nibbles of 93% of the inner nodes and 86% of the leaves, and every root. The nodes that differ differ
//! by one nibble in all but 1.4%, and the stored side of such a node lies within half a codec step of the boundary
//! between the two nibbles: the files' trees were built from the vertices before they were quantised, and the rule is
//! applied here to the vertices after. The topology is not a rule this crate reproduces: the stock trees split neither
//! at the median nor along the longest axis, and their children overlap on the split axis in 95% of the inner nodes.
//! [`balanced_shape`] makes a median split, which any reader walks the same way.
#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]
use crate::bvtree::{BvNodeKind, BvTree, NodeWidth};
use crate::{Result, invalid};

/// A box, minimum then maximum (xyz).
pub type Aabb = [[f64; 3]; 2];

/// The divisor of the nibble squares.
const NIBBLE_DIVISOR: f64 = 226.0;

/// One node of a tree's shape: the tree without its boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// An internal node; its first child is the next node, its second is at this index.
    Internal {
        /// The index of the second child.
        second_child: usize,
    },
    /// A leaf for a primitive (section trees, extern aabb trees) or a section (mesh trees).
    Leaf {
        /// The primitive or section index.
        payload: u32,
    },
}

/// The shape of a decoded tree.
pub fn shape_of(tree: &BvTree) -> Vec<Shape> {
    tree.nodes()
        .iter()
        .map(|n| match n.kind {
            BvNodeKind::Internal { second_child } => Shape::Internal { second_child },
            BvNodeKind::Leaf { payload } => Shape::Leaf { payload },
        })
        .collect()
}

/// An encoded tree.
#[derive(Debug, Clone, PartialEq)]
pub struct EncodedTree {
    /// The nodes, `width` bytes each.
    pub bytes: Vec<u8>,
    /// The box each node decodes to (the box [`BvTree::decode`] computes from the same bytes and domain).
    pub boxes: Vec<Aabb>,
    /// Each leaf's payload and node index, in node order.
    pub leaves: Vec<(u32, usize)>,
}

/// The union of two boxes.
pub fn union(a: &Aabb, b: &Aabb) -> Aabb {
    let mut out = *a;
    for axis in 0..3 {
        if let (Some(lo), Some(hi), Some(blo), Some(bhi)) = (
            out[0].get_mut(axis),
            out[1].get_mut(axis),
            b[0].get(axis),
            b[1].get(axis),
        ) {
            if *blo < *lo {
                *lo = *blo;
            }
            if *bhi > *hi {
                *hi = *bhi;
            }
        }
    }
    out
}

/// A nibble guess: the value clamped to 0 to 15.
#[allow(clippy::cast_possible_truncation, reason = "clamped to 0..=15 first")]
fn small(x: f64) -> u8 {
    x.clamp(0.0, 15.0) as u8
}

/// The largest nibble (0 to 15) whose side lies no further in than `target`: `parent_edge + extent × n² / 226` (the
/// minimum side, `from_min`) or `parent_edge - extent × n² / 226` (the maximum side) tested with the expression the
/// decoder evaluates.
fn nibble(parent_edge: f64, extent: f64, target: f64, from_min: bool) -> u8 {
    let gap = if from_min {
        target - parent_edge
    } else {
        parent_edge - target
    };
    if extent.is_nan() || gap.is_nan() || extent <= 0.0 || gap <= 0.0 {
        return 0;
    }
    let mut n = small((NIBBLE_DIVISOR * gap / extent).sqrt().floor());
    // the decoder's own arithmetic decides: step down until the side is not past the target
    while n > 0 {
        let step = extent * f64::from(n) * f64::from(n) / NIBBLE_DIVISOR;
        let side = if from_min {
            parent_edge + step
        } else {
            parent_edge - step
        };
        let inside = if from_min { side <= target } else { side >= target };
        if inside {
            break;
        }
        n = n.saturating_sub(1);
    }
    n
}

/// The three axis bytes of a node whose subtree needs `tight` under a parent box `parent`, and the box they decode to.
fn axes_for(parent: &Aabb, tight: &Aabb) -> ([u8; 3], Aabb) {
    let mut bytes = [0u8; 3];
    let mut decoded = *parent;
    for axis in 0..3 {
        let (Some(plo), Some(phi), Some(tlo), Some(thi)) = (
            parent[0].get(axis).copied(),
            parent[1].get(axis).copied(),
            tight[0].get(axis).copied(),
            tight[1].get(axis).copied(),
        ) else {
            continue;
        };
        let extent = phi - plo;
        let h = nibble(plo, extent, tlo, true);
        let l = nibble(phi, extent, thi, false);
        if let Some(b) = bytes.get_mut(axis) {
            *b = h << 4 | l;
        }
        if let (Some(lo), Some(hi)) = (decoded[0].get_mut(axis), decoded[1].get_mut(axis)) {
            // exactly the decoder's expression
            *lo = plo + extent * f64::from(h) * f64::from(h) / NIBBLE_DIVISOR;
            *hi = phi - extent * f64::from(l) * f64::from(l) / NIBBLE_DIVISOR;
        }
    }
    (bytes, decoded)
}

/// The data bytes of a node: an internal node's second child `offset` nodes on, or a leaf's `payload`. Refused when the
/// value does not fit the width.
fn data_bytes(width: NodeWidth, shape: Shape, index: usize) -> Result<Vec<u8>> {
    let too_big = |what: &str| {
        invalid(
            index,
            format!("a tree node's {what} does not fit its {} bytes", width.bytes()),
        )
    };
    match (width, shape) {
        (NodeWidth::Four, Shape::Internal { second_child }) => {
            let offset = second_child
                .checked_sub(index)
                .filter(|o| o & 1 == 0)
                .ok_or_else(|| too_big("second child offset"))?;
            let byte = u8::try_from(offset)
                .ok()
                .filter(|b| b & 1 == 0)
                .ok_or_else(|| too_big("second child offset"))?;
            Ok(vec![byte | 1])
        }
        (NodeWidth::Four, Shape::Leaf { payload }) => {
            let byte = u8::try_from(payload)
                .ok()
                .filter(|p| *p < 128)
                .ok_or_else(|| too_big("primitive index"))?;
            Ok(vec![byte << 1])
        }
        (NodeWidth::Five, Shape::Internal { second_child }) => {
            let offset = second_child
                .checked_sub(index)
                .filter(|o| o & 1 == 0)
                .ok_or_else(|| too_big("second child offset"))?;
            let half = u16::try_from(offset >> 1)
                .ok()
                .filter(|h| *h < 0x8000)
                .ok_or_else(|| too_big("second child offset"))?;
            Ok((half | 0x8000).to_be_bytes().to_vec())
        }
        (NodeWidth::Five, Shape::Leaf { payload }) => {
            let value = u16::try_from(payload)
                .ok()
                .filter(|p| *p < 0x8000)
                .ok_or_else(|| too_big("section index"))?;
            Ok(value.to_be_bytes().to_vec())
        }
        (NodeWidth::Six, Shape::Internal { second_child }) => {
            let offset = second_child
                .checked_sub(index)
                .filter(|o| o & 1 == 0)
                .ok_or_else(|| too_big("second child offset"))?;
            let half = u32::try_from(offset >> 1)
                .ok()
                .filter(|h| *h < 0x80_0000)
                .ok_or_else(|| too_big("second child offset"))?;
            six(0x80, half)
        }
        (NodeWidth::Six, Shape::Leaf { payload }) => {
            if payload >= 0x80_0000 {
                return Err(too_big("primitive index"));
            }
            six(0, payload)
        }
    }
}

/// `hiData` and the little-endian `loData` of a 23-bit `value` with the high flag byte `flag`.
fn six(flag: u8, value: u32) -> Result<Vec<u8>> {
    let hi = u8::try_from(value >> 16).map_err(|_| invalid(0, "a 23-bit value that is wider"))?;
    let [low, high, ..] = (value & 0xffff).to_le_bytes();
    Ok(vec![flag | hi, low, high])
}

/// Encode a tree of `shape` with the box `leaf_box(payload)` under each leaf, rooted at `domain`.
///
/// Each node's nibbles are the largest that keep its box inside what lies below it; the box a node decodes
/// to is then at least the union of its children's boxes grown back to the nodes' own, so every leaf's decoded box holds
/// the box it was given (to the rounding of the decoder's `f64` arithmetic).
///
/// Refused: a shape that is not a pre-order binary tree (a second child outside the node's subtree, a leaf that does not
/// end its subtree), a leaf without a box, a value too large for the node width, a domain that is not finite and ordered.
pub fn encode_tree(
    width: NodeWidth,
    domain: [[f32; 3]; 2],
    shape: &[Shape],
    leaf_box: &dyn Fn(u32) -> Option<Aabb>,
) -> Result<EncodedTree> {
    let count = shape.len();
    let root: Aabb = domain.map(|corner| corner.map(f64::from));
    if root.iter().flatten().any(|c| !c.is_finite()) || root[0].iter().zip(&root[1]).any(|(lo, hi)| lo > hi) {
        return Err(invalid(
            0,
            format!("a tree cannot be rooted at the domain {domain:?}"),
        ));
    }
    // the tight box under each node: children come after their parent, so fill from the back
    let mut tight: Vec<Aabb> = vec![root; count];
    for i in (0..count).rev() {
        let node = match shape.get(i).copied() {
            Some(Shape::Leaf { payload }) => leaf_box(payload)
                .ok_or_else(|| invalid(i, format!("no box for the leaf of payload {payload}")))?,
            Some(Shape::Internal { second_child }) => {
                let first = tight.get(i.saturating_add(1)).copied();
                let second = tight.get(second_child).copied();
                match (first, second) {
                    (Some(a), Some(b)) if second_child > i.saturating_add(1) => union(&a, &b),
                    _ => {
                        return Err(invalid(
                            i,
                            format!("node {i}: its second child {second_child} is not a later node"),
                        ));
                    }
                }
            }
            None => return Err(invalid(i, "a node past the shape")),
        };
        if let Some(slot) = tight.get_mut(i) {
            *slot = node;
        }
    }
    let node_bytes = width.bytes();
    let mut bytes = vec![0u8; count.saturating_mul(node_bytes)];
    let mut boxes: Vec<Aabb> = vec![root; count];
    let mut leaves: Vec<(u32, usize)> = Vec::new();
    // (node, end of its subtree, the parent's decoded box); the second child is pushed first so the first pops first
    let mut stack: Vec<(usize, usize, Aabb)> = Vec::new();
    if count > 0 {
        stack.push((0, count, root));
    }
    let mut reached = 0usize;
    while let Some((i, end, parent)) = stack.pop() {
        let this = shape.get(i).copied().filter(|_| i < end);
        let Some(this) = this else {
            return Err(invalid(
                i,
                format!("node {i} lies outside its subtree, which ends at {end}"),
            ));
        };
        let tight_here = tight.get(i).copied().unwrap_or(root);
        let (axes, decoded) = axes_for(&parent, &tight_here);
        let data = data_bytes(width, this, i)?;
        let at = i.saturating_mul(node_bytes);
        let [a, b, c] = axes;
        let mut node = vec![a, b, c];
        node.extend(data);
        if node.len() != node_bytes {
            return Err(invalid(i, "a node of the wrong width"));
        }
        bytes
            .get_mut(at..at.saturating_add(node_bytes))
            .ok_or_else(|| invalid(i, "a node past the bytes"))?
            .copy_from_slice(&node);
        if let Some(slot) = boxes.get_mut(i) {
            *slot = decoded;
        }
        reached = reached.saturating_add(1);
        match this {
            Shape::Internal { second_child } => {
                if second_child <= i.saturating_add(1) || second_child >= end {
                    return Err(invalid(
                        i,
                        format!(
                            "node {i}: its second child at {second_child} is not inside its subtree, which ends at {end}"
                        ),
                    ));
                }
                stack.push((second_child, end, decoded));
                stack.push((i.saturating_add(1), second_child, decoded));
            }
            Shape::Leaf { payload } => {
                if i.saturating_add(1) != end {
                    return Err(invalid(
                        i,
                        format!("leaf {i} does not end its subtree, which ends at {end}"),
                    ));
                }
                leaves.push((payload, i));
            }
        }
    }
    if reached != count {
        return Err(invalid(0, format!("{reached} of {count} nodes reached")));
    }
    leaves.sort_unstable();
    if let Some(pair) = leaves
        .windows(2)
        .find(|w| w.first().map(|a| a.0) == w.get(1).map(|b| b.0))
    {
        let payload = pair.first().map_or(0, |a| a.0);
        return Err(invalid(0, format!("two leaves name {payload}")));
    }
    // back in node order
    leaves.sort_unstable_by_key(|&(_, node)| node);
    Ok(EncodedTree { bytes, boxes, leaves })
}

/// A shape for the boxes `boxes`: split the leaves in two at the median along the longest axis of their centres, and
/// each half again. Leaf payloads count up in pre-order, so the leaf of payload `k` is the `k`-th leaf. Returns the shape
/// and, for each payload, the index of the box it stands for.
///
/// No boxes make an empty shape. Refused: a box with a non-finite side.
pub fn balanced_shape(boxes: &[Aabb]) -> Result<(Vec<Shape>, Vec<usize>)> {
    if boxes.iter().flatten().flatten().any(|c| !c.is_finite()) {
        return Err(invalid(0, "a box with a side that is not finite"));
    }
    let centre = |i: usize, axis: usize| -> f64 {
        boxes.get(i).map_or(0.0, |b| {
            (b[0].get(axis).copied().unwrap_or(0.0) + b[1].get(axis).copied().unwrap_or(0.0)) * 0.5
        })
    };
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    let mut shape: Vec<Shape> = Vec::with_capacity(boxes.len().saturating_mul(2));
    let mut leaves: Vec<usize> = Vec::with_capacity(boxes.len());
    enum Frame {
        Range { start: usize, end: usize },
        Second { node: usize, start: usize, end: usize },
    }
    let mut stack = vec![Frame::Range {
        start: 0,
        end: boxes.len(),
    }];
    while let Some(frame) = stack.pop() {
        let (start, end) = match frame {
            Frame::Range { start, end } => (start, end),
            Frame::Second { node, start, end } => {
                let next = shape.len();
                if let Some(Shape::Internal { second_child }) = shape.get_mut(node) {
                    *second_child = next;
                }
                (start, end)
            }
        };
        let len = end.saturating_sub(start);
        if len == 0 {
            continue;
        }
        if len == 1 {
            let payload = u32::try_from(leaves.len()).map_err(|_| invalid(0, "more than 2^32 leaves"))?;
            shape.push(Shape::Leaf { payload });
            if let Some(&item) = order.get(start) {
                leaves.push(item);
            }
            continue;
        }
        // the longest axis of the centres of this range
        let mut low = [f64::MAX; 3];
        let mut high = [f64::MIN; 3];
        for &item in order.get(start..end).unwrap_or(&[]) {
            for axis in 0..3 {
                let c = centre(item, axis);
                if let (Some(l), Some(h)) = (low.get_mut(axis), high.get_mut(axis)) {
                    *l = l.min(c);
                    *h = h.max(c);
                }
            }
        }
        let axis = (0..3)
            .max_by(|&a, &b| {
                let ea = high.get(a).copied().unwrap_or(0.0) - low.get(a).copied().unwrap_or(0.0);
                let eb = high.get(b).copied().unwrap_or(0.0) - low.get(b).copied().unwrap_or(0.0);
                ea.partial_cmp(&eb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(0);
        if let Some(range) = order.get_mut(start..end) {
            range.sort_unstable_by(|&a, &b| {
                centre(a, axis)
                    .partial_cmp(&centre(b, axis))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.cmp(&b))
            });
        }
        let mid = start.saturating_add(len >> 1);
        let node = shape.len();
        shape.push(Shape::Internal { second_child: 0 });
        stack.push(Frame::Second {
            node,
            start: mid,
            end,
        });
        stack.push(Frame::Range { start, end: mid });
    }
    Ok((shape, leaves))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "a test reads what it just built"
)]
mod tests {
    use super::*;

    /// A box whose extent is 226 on every axis, so a nibble `n` moves a side by exactly `n * n`.
    const DOMAIN: [[f32; 3]; 2] = [[0.0; 3], [226.0; 3]];

    fn boxed(lo: [f64; 3], hi: [f64; 3]) -> Aabb {
        [lo, hi]
    }

    /// A tree of `leaves` leaves whose every internal node has an internal first child (but the last), so the root's
    /// second child is `2 * (leaves - 1)` nodes on: the internal nodes first, then the leaves.
    fn left_comb(leaves: u32) -> Vec<Shape> {
        let internal = (leaves - 1) as usize;
        let mut shape = Vec::new();
        for k in 0..internal {
            shape.push(Shape::Internal {
                second_child: 2 * (internal - k) + k,
            });
        }
        for payload in 0..leaves {
            shape.push(Shape::Leaf { payload });
        }
        shape
    }

    #[test]
    fn a_nibble_is_the_largest_that_stays_inside() {
        // extent 226: the minimum side moves n * n
        assert_eq!(nibble(0.0, 226.0, 0.0, true), 0);
        assert_eq!(nibble(0.0, 226.0, 3.9, true), 1);
        assert_eq!(nibble(0.0, 226.0, 4.0, true), 2);
        assert_eq!(nibble(0.0, 226.0, 80.0, true), 8);
        assert_eq!(nibble(0.0, 226.0, 225.0, true), 15);
        assert_eq!(nibble(0.0, 226.0, 1e9, true), 15);
        // the maximum side moves in from 226
        assert_eq!(nibble(226.0, 226.0, 226.0, false), 0);
        assert_eq!(nibble(226.0, 226.0, 222.0, false), 2);
        assert_eq!(nibble(226.0, 226.0, 222.5, false), 1);
        // a flat parent, or a target outside it, takes nothing
        assert_eq!(nibble(5.0, 0.0, 5.0, true), 0);
        assert_eq!(nibble(0.0, 226.0, -10.0, true), 0);
    }

    #[test]
    fn a_two_leaf_tree_encodes_and_decodes_to_boxes_that_hold_its_leaves() {
        let boxes = [
            boxed([1.0, 1.0, 1.0], [10.0, 10.0, 10.0]),
            boxed([100.0, 4.0, 0.0], [200.0, 225.0, 226.0]),
        ];
        let shape = [
            Shape::Internal { second_child: 2 },
            Shape::Leaf { payload: 0 },
            Shape::Leaf { payload: 1 },
        ];
        let leaf = |p: u32| boxes.get(p as usize).copied();
        for width in [NodeWidth::Four, NodeWidth::Five, NodeWidth::Six] {
            let tree = encode_tree(width, DOMAIN, &shape, &leaf).unwrap();
            assert_eq!(tree.bytes.len(), 3 * width.bytes());
            let decoded = BvTree::decode(&tree.bytes, width, DOMAIN, 0).unwrap();
            assert_eq!(shape_of(&decoded), shape);
            assert_eq!(tree.leaves, [(0, 1), (1, 2)]);
            for (i, node) in decoded.nodes().iter().enumerate() {
                // the encoder's boxes are the decoder's, bit for bit
                assert_eq!(node.aabb, tree.boxes[i], "{width:?} node {i}");
            }
            // each leaf's box holds the box it was given
            for (p, want) in boxes.iter().enumerate() {
                let got = &decoded.nodes()[tree.leaves[p].1].aabb;
                for axis in 0..3 {
                    assert!(
                        got[0][axis] <= want[0][axis] && got[1][axis] >= want[1][axis],
                        "{width:?} leaf {p}"
                    );
                }
            }
        }
    }

    #[test]
    fn stock_nibbles_are_chosen_by_the_floor_rule() {
        // a root over the domain whose first child needs [4, 4, 0]..[225, 226, 226]: the minimum side moves 4 = 2 * 2
        let boxes = [boxed([4.0, 4.0, 0.0], [225.0, 226.0, 226.0])];
        let shape = [Shape::Leaf { payload: 0 }];
        let tree = encode_tree(NodeWidth::Four, DOMAIN, &shape, &|p| {
            boxes.get(p as usize).copied()
        })
        .unwrap();
        // x: min 4 -> 2, max 225 -> 226 - 1 -> l = 1; y: 2 and 0; z: 0 and 0
        assert_eq!(tree.bytes[..3], [0x21, 0x20, 0x00]);
        assert_eq!(tree.bytes[3], 0, "the lone leaf names payload 0");
    }

    #[test]
    fn values_too_large_for_the_width_are_refused() {
        let boxes = [boxed([0.0; 3], [1.0; 3])];
        let leaf = |_: u32| Some(boxes[0]);
        // a leaf payload of 128 does not fit a 4-byte node's seven bits
        let e = encode_tree(NodeWidth::Four, DOMAIN, &[Shape::Leaf { payload: 128 }], &leaf)
            .unwrap_err()
            .to_string();
        assert!(e.contains("primitive index"), "{e}");
        // a second child 254 nodes on is the furthest a 4-byte node reaches, 256 is too far
        assert!(encode_tree(NodeWidth::Four, DOMAIN, &left_comb(128), &leaf).is_ok());
        let e = encode_tree(NodeWidth::Four, DOMAIN, &left_comb(129), &leaf)
            .unwrap_err()
            .to_string();
        assert!(e.contains("second child offset"), "{e}");
        // a shape whose second child is not inside the tree
        let e = encode_tree(
            NodeWidth::Five,
            DOMAIN,
            &[Shape::Internal { second_child: 5 }],
            &leaf,
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("not a later node"), "{e}");
        // a leaf without a box
        let e = encode_tree(NodeWidth::Five, DOMAIN, &[Shape::Leaf { payload: 0 }], &|_| None)
            .unwrap_err()
            .to_string();
        assert!(e.contains("no box for the leaf"), "{e}");
    }

    #[test]
    fn a_balanced_shape_visits_every_box_once_and_numbers_leaves_in_pre_order() {
        let boxes: Vec<Aabb> = (0..37)
            .map(|i| {
                let x = f64::from((i * 7919) % 101);
                boxed([x, 0.0, 0.0], [x + 1.0, 1.0, 1.0])
            })
            .collect();
        let (shape, leaves) = balanced_shape(&boxes).unwrap();
        assert_eq!(shape.len(), 2 * 37 - 1);
        let mut seen = leaves.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..37).collect::<Vec<_>>());
        let payloads: Vec<u32> = shape
            .iter()
            .filter_map(|s| {
                if let Shape::Leaf { payload } = s {
                    Some(*payload)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(payloads, (0..37).collect::<Vec<_>>());
        // the shape is a valid tree: encoding it succeeds, and it decodes back to itself
        let leaf = |p: u32| leaves.get(p as usize).and_then(|&i| boxes.get(i).copied());
        let domain = [[0.0f32, 0.0, 0.0], [110.0, 1.0, 1.0]];
        let tree = encode_tree(NodeWidth::Four, domain, &shape, &leaf).unwrap();
        let decoded = BvTree::decode(&tree.bytes, NodeWidth::Four, domain, 0).unwrap();
        assert_eq!(shape_of(&decoded), shape);
        // a single box, and none
        assert_eq!(
            balanced_shape(&boxes[..1]).unwrap().0,
            [Shape::Leaf { payload: 0 }]
        );
        assert!(balanced_shape(&[]).unwrap().0.is_empty());
        // a non-finite box
        assert!(balanced_shape(&[boxed([f64::NAN; 3], [0.0; 3])]).is_err());
    }
}
