/*
 * Copyright (c) Qualcomm Technologies, Inc. and/or its subsidiaries.
 * SPDX-License-Identifier: BSD-3-Clause-Clear
 */

//! Timeout min-heap for timer management.
//!
//! Timeout entries are kept in a `BinaryHeap` keyed by expiration time
//! (ticks), so the soonest timeout is always at the head. `remove` (needed
//! when a thread's timeout is cancelled or replaced before it expires) has
//! no O(1) equivalent on a `BinaryHeap`; since removal only happens on
//! `SetTimeout`/`DeltaTimeout` — not once per tick — it rebuilds the heap
//! via drain-and-filter rather than tracking a side table.

use alloc::collections::BinaryHeap;
use core::cmp::Reverse;

/// Index of a tree node, or `IDX_NONE` for null.
pub type TreeIdx = u32;

/// Sentinel value for "no node".
pub const IDX_NONE: TreeIdx = 0;

/// A node in the timeout tree.
///
/// Each thread context embeds a TreeNode for its timeout.
#[derive(Clone, Copy, Debug, Default)]
pub struct TreeNode {
    pub left: TreeIdx,
    pub right: TreeIdx,
    pub key: u64,
}

/// Heap entry: `(expiration key, thread index)`. Lower key sorts first
/// (soonest timeout).
type Entry = Reverse<(u64, TreeIdx)>;

/// A timeout min-heap, keyed by expiration ticks.
#[derive(Debug, Default)]
pub struct TimeoutTree {
    heap: BinaryHeap<Entry>,
}

impl TimeoutTree {
    pub const fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
        }
    }

    /// Insert a node into the tree with the given key.
    pub fn add(&mut self, nodes: &mut [TreeNode], idx: TreeIdx, key: u64) {
        nodes[idx as usize].key = key;
        nodes[idx as usize].left = IDX_NONE;
        nodes[idx as usize].right = IDX_NONE;
        self.heap.push(Reverse((key, idx)));
    }

    /// Remove a node with the given key from the tree.
    pub fn remove(&mut self, _nodes: &mut [TreeNode], idx: TreeIdx, key: u64) {
        let remaining: BinaryHeap<Entry> = self
            .heap
            .drain()
            .filter(|&Reverse((k, i))| !(k == key && i == idx))
            .collect();
        self.heap = remaining;
    }

    /// Find the minimum key node (soonest timeout).
    ///
    /// Returns the node index, or `IDX_NONE` if empty.
    pub fn min(&self, nodes: &[TreeNode]) -> TreeIdx {
        let _ = nodes;
        match self.heap.peek() {
            Some(&Reverse((_, idx))) => idx,
            None => IDX_NONE,
        }
    }

    /// Iterate all nodes in ascending-key order, collecting indices.
    ///
    /// This destroys the tree (drains the heap).
    pub fn collect_and_clear(&mut self, nodes: &[TreeNode], out: &mut impl FnMut(TreeIdx)) {
        let _ = nodes;
        let mut entries: alloc::vec::Vec<Entry> = self.heap.drain().collect();
        entries.sort_by_key(|&Reverse((key, idx))| (key, idx));
        for Reverse((_, idx)) in entries {
            out(idx);
        }
    }

    /// Pop all entries with key `<= split_key` (soonest first), invoking
    /// `expired` for each. Returns the key of the next remaining (pending)
    /// timeout, or `None` if the tree is now empty.
    pub fn pop_expired(
        &mut self,
        split_key: u64,
        expired: &mut impl FnMut(TreeIdx),
    ) -> Option<u64> {
        while let Some(&Reverse((key, idx))) = self.heap.peek() {
            if key > split_key {
                break;
            }
            self.heap.pop();
            expired(idx);
        }
        self.heap.peek().map(|&Reverse((key, _))| key)
    }

    /// Check if the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn make_nodes(n: usize) -> Vec<TreeNode> {
        let mut v = Vec::new();
        v.resize(n, TreeNode::default());
        v
    }

    #[test]
    fn test_empty_tree() {
        let tree = TimeoutTree::new();
        let nodes = make_nodes(4);
        assert!(tree.is_empty());
        assert_eq!(tree.min(&nodes), IDX_NONE);
    }

    #[test]
    fn test_add_and_min() {
        let mut tree = TimeoutTree::new();
        let mut nodes = make_nodes(8);

        tree.add(&mut nodes, 1, 100);
        tree.add(&mut nodes, 2, 50);
        tree.add(&mut nodes, 3, 200);
        tree.add(&mut nodes, 4, 75);

        assert_eq!(tree.min(&nodes), 2); // key=50 is min
    }

    #[test]
    fn test_remove() {
        let mut tree = TimeoutTree::new();
        let mut nodes = make_nodes(8);

        tree.add(&mut nodes, 1, 100);
        tree.add(&mut nodes, 2, 50);
        tree.add(&mut nodes, 3, 200);

        // Remove min
        tree.remove(&mut nodes, 2, 50);
        assert_eq!(tree.min(&nodes), 1); // key=100 is now min

        // Remove root
        tree.remove(&mut nodes, 1, 100);
        assert_eq!(tree.min(&nodes), 3); // only key=200 left

        // Remove last
        tree.remove(&mut nodes, 3, 200);
        assert!(tree.is_empty());
    }

    #[test]
    fn test_collect_and_clear() {
        let mut tree = TimeoutTree::new();
        let mut nodes = make_nodes(8);

        tree.add(&mut nodes, 3, 30);
        tree.add(&mut nodes, 1, 10);
        tree.add(&mut nodes, 2, 20);

        let mut items = Vec::new();
        tree.collect_and_clear(&nodes, &mut |idx| items.push(idx));

        // Should be in-order by key
        assert_eq!(items.len(), 3);
        assert_eq!(nodes[items[0] as usize].key, 10);
        assert_eq!(nodes[items[1] as usize].key, 20);
        assert_eq!(nodes[items[2] as usize].key, 30);
        assert!(tree.is_empty());
    }

    #[test]
    fn test_duplicate_keys() {
        let mut tree = TimeoutTree::new();
        let mut nodes = make_nodes(8);

        tree.add(&mut nodes, 1, 100);
        tree.add(&mut nodes, 2, 100);
        tree.add(&mut nodes, 3, 100);

        // All three should be present
        let mut items = Vec::new();
        tree.collect_and_clear(&nodes, &mut |idx| items.push(idx));
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_pop_expired_partial() {
        let mut tree = TimeoutTree::new();
        let mut nodes = make_nodes(8);

        tree.add(&mut nodes, 1, 10);
        tree.add(&mut nodes, 2, 20);
        tree.add(&mut nodes, 3, 30);
        tree.add(&mut nodes, 4, 40);
        tree.add(&mut nodes, 5, 50);

        // Split at key=25: expired={10,20}, pending next key=30
        let mut expired = Vec::new();
        let next_key = tree.pop_expired(25, &mut |idx| expired.push(idx));
        assert_eq!(expired.len(), 2);
        assert!(expired.iter().all(|&i| nodes[i as usize].key <= 25));
        assert_eq!(next_key, Some(30));
        assert_eq!(tree.min(&nodes), 3); // key=30 is min of what remains
    }

    #[test]
    fn test_pop_expired_all() {
        let mut tree = TimeoutTree::new();
        let mut nodes = make_nodes(8);

        tree.add(&mut nodes, 1, 10);
        tree.add(&mut nodes, 2, 20);

        let mut expired = Vec::new();
        let next_key = tree.pop_expired(100, &mut |idx| expired.push(idx));
        assert_eq!(expired.len(), 2);
        assert_eq!(next_key, None);
        assert!(tree.is_empty());
    }

    #[test]
    fn test_pop_expired_none() {
        let mut tree = TimeoutTree::new();
        let mut nodes = make_nodes(8);

        tree.add(&mut nodes, 1, 100);
        tree.add(&mut nodes, 2, 200);

        let mut expired = Vec::new();
        let next_key = tree.pop_expired(50, &mut |idx| expired.push(idx));
        assert!(expired.is_empty());
        assert_eq!(next_key, Some(100));
        assert_eq!(tree.min(&nodes), 1); // key=100
    }
}
