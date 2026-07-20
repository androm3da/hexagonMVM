/*
 * Copyright (c) Qualcomm Technologies, Inc. and/or its subsidiaries.
 * SPDX-License-Identifier: BSD-3-Clause-Clear
 */

use crate::debug;
use minivm_timer::tree::{TimeoutTree, TreeIdx, TreeNode, IDX_NONE};

pub fn run() {
    debug::write0(b"  [test] timeout tree\n\0");
    {
        let mut nodes: [TreeNode; 8] = [TreeNode::default(); 8];
        let mut tree = TimeoutTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.min(&nodes), IDX_NONE);

        // Add nodes with different keys
        tree.add(&mut nodes, 1, 100);
        tree.add(&mut nodes, 2, 50);
        tree.add(&mut nodes, 3, 200);
        tree.add(&mut nodes, 4, 75);

        assert!(!tree.is_empty());
        assert_eq!(tree.min(&nodes), 2); // key=50 is min

        // Remove the minimum
        tree.remove(&mut nodes, 2, 50);
        assert_eq!(tree.min(&nodes), 4); // key=75 is new min

        // Remove root-ish node
        tree.remove(&mut nodes, 1, 100);
        assert_eq!(tree.min(&nodes), 4); // key=75 still min

        // Remove all
        tree.remove(&mut nodes, 4, 75);
        tree.remove(&mut nodes, 3, 200);
        assert!(tree.is_empty());

        // Test pop_expired: split into expired vs pending
        tree.add(&mut nodes, 1, 10);
        tree.add(&mut nodes, 2, 20);
        tree.add(&mut nodes, 3, 30);
        tree.add(&mut nodes, 4, 40);
        tree.add(&mut nodes, 5, 50);

        // Split at key=25: expired={10,20}, pending next key=30
        let mut expired: [TreeIdx; 8] = [IDX_NONE; 8];
        let mut expired_count = 0;
        let next_key = tree.pop_expired(25, &mut |idx| {
            expired[expired_count] = idx;
            expired_count += 1;
        });
        assert_eq!(expired_count, 2);
        assert!(expired[..expired_count]
            .iter()
            .all(|&i| nodes[i as usize].key <= 25));
        assert_eq!(next_key, Some(30));
        assert_eq!(tree.min(&nodes), 3); // key=30 is min of what remains

        // Test pop_expired with all expired
        let mut tree2 = TimeoutTree::new();
        tree2.add(&mut nodes, 6, 5);
        tree2.add(&mut nodes, 7, 15);
        let next_key2 = tree2.pop_expired(100, &mut |_| {});
        assert_eq!(next_key2, None);
        assert!(tree2.is_empty());
    }
    debug::write0(b"  [test] timeout tree OK\n\0");
}
