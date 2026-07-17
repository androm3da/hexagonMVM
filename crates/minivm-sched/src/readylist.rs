/*
 * Copyright (c) Qualcomm Technologies, Inc. and/or its subsidiaries.
 * SPDX-License-Identifier: BSD-3-Clause-Clear
 */

//! Priority-based ready queue.
//!
//! The ready list is a priority queue backed by a `BinaryHeap` keyed on
//! `(priority, sequence)`, with `sequence` breaking ties in FIFO order
//! (lower sequence = scheduled first). `append` assigns an increasing
//! sequence number; `insert` (front-of-queue) assigns a decreasing one, so
//! it always sorts ahead of any already-appended entry at the same
//! priority.
//!
//! Removal by thread identity (`remove`) — needed when a thread blocks or
//! is killed out of order, not just when the best-priority thread is
//! popped — has no O(1) equivalent on a `BinaryHeap`. Since this is the
//! hottest operation here (called on every context switch from the
//! scheduler), it uses lazy deletion: a `removed` side bitmap is stamped
//! O(1), and stale (removed) entries are discarded lazily as they are
//! popped off the heap in `getbest`.
//!
//! Index 0 is reserved as "null" (no thread). Valid thread indices start at 1.

use crate::context::ThreadContext;
use alloc::collections::BinaryHeap;
use core::cmp::Reverse;
use minivm_types::consts::MAX_PRIOS;
use minivm_types::vm::ThreadStatus;

/// Sentinel index meaning "no thread" (equivalent to NULL pointer).
pub const IDX_NONE: u32 = 0;

/// Number of u32 words in the removed-thread bitmap.
const REMOVED_WORDS: usize = 256 / 32;

/// Heap entry: `(priority, sequence, thread index)`. Ordered so that the
/// numerically lowest priority (highest scheduling priority) sorts first,
/// with ties broken by sequence (lowest first, i.e. FIFO for `append`).
type Entry = Reverse<(u8, i64, u32)>;

/// Ready list: priority-based queue of threads.
pub struct ReadyList {
    /// Min-heap of ready threads, keyed by (priority, sequence).
    heap: BinaryHeap<Entry>,
    /// Monotonically increasing counter for FIFO ordering on `append`.
    next_seq: i64,
    /// Monotonically decreasing counter so `insert` always sorts ahead of
    /// same-priority entries already in the heap.
    prev_seq: i64,
    /// Lazy-deletion bitmap: bit N set means thread index N was removed
    /// and its heap entry (if still present) must be discarded on pop.
    removed: [u32; REMOVED_WORDS],
}

impl Default for ReadyList {
    fn default() -> Self {
        Self::new()
    }
}

impl ReadyList {
    /// Create an empty ready list.
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            next_seq: 0,
            prev_seq: -1,
            removed: [0u32; REMOVED_WORDS],
        }
    }

    fn is_removed(&self, idx: u32) -> bool {
        let word = (idx >> 5) as usize;
        let bit = idx & 0x1f;
        (self.removed[word] >> bit) & 1 != 0
    }

    fn set_removed(&mut self, idx: u32, removed: bool) {
        let word = (idx >> 5) as usize;
        let bit = idx & 0x1f;
        if removed {
            self.removed[word] |= 1u32 << bit;
        } else {
            self.removed[word] &= !(1u32 << bit);
        }
    }

    /// Discard stale (removed) entries from the top of the heap.
    fn drain_removed(&mut self) {
        while let Some(&Reverse((_, _, idx))) = self.heap.peek() {
            if self.is_removed(idx) {
                self.heap.pop();
                self.set_removed(idx, false);
            } else {
                break;
            }
        }
    }

    /// Find the highest (numerically lowest) priority that has a ready thread.
    ///
    /// Returns `MAX_PRIOS` if no threads are ready.
    pub fn best_prio(&mut self) -> u32 {
        self.drain_removed();
        match self.heap.peek() {
            Some(&Reverse((prio, _, _))) => prio as u32,
            None => MAX_PRIOS,
        }
    }

    /// Check whether any threads are ready.
    pub fn any_valid(&mut self) -> bool {
        self.best_prio() < MAX_PRIOS
    }

    /// Check whether a thread at a given priority is ready.
    pub fn prio_valid(&mut self, prio: u32) -> bool {
        self.drain_removed();
        self.heap
            .iter()
            .any(|&Reverse((p, _, idx))| p as u32 == prio && !self.is_removed(idx))
    }

    /// Append a thread to the end of its priority ring (last to be scheduled).
    ///
    /// Sets the thread's status to Ready.
    pub fn append(&mut self, threads: &mut [ThreadContext], idx: u32) {
        let prio = threads[idx as usize].prio;
        threads[idx as usize].status = ThreadStatus::Ready as u8;
        let seq = self.next_seq;
        self.next_seq += 1;
        self.set_removed(idx, false);
        self.heap.push(Reverse((prio, seq, idx)));
    }

    /// Insert a thread at the front of its priority ring (first to be scheduled).
    ///
    /// Sets the thread's status to Ready.
    pub fn insert(&mut self, threads: &mut [ThreadContext], idx: u32) {
        let prio = threads[idx as usize].prio;
        threads[idx as usize].status = ThreadStatus::Ready as u8;
        let seq = self.prev_seq;
        self.prev_seq -= 1;
        self.set_removed(idx, false);
        self.heap.push(Reverse((prio, seq, idx)));
    }

    /// Remove a specific thread from the ready list.
    ///
    /// The caller guarantees that the thread is actually in the ready list.
    /// O(1): marks the thread as removed; its stale heap entry is
    /// discarded lazily on the next pop that reaches it.
    pub fn remove(&mut self, _threads: &mut [ThreadContext], idx: u32) {
        self.set_removed(idx, true);
    }

    /// Remove and return the best (highest-priority) ready thread.
    ///
    /// Returns `IDX_NONE` if no threads are ready.
    pub fn getbest(&mut self, _threads: &mut [ThreadContext]) -> u32 {
        self.drain_removed();
        match self.heap.pop() {
            Some(Reverse((_, _, idx))) => idx,
            None => IDX_NONE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a thread array with `n` zeroed contexts.
    /// Index 0 is reserved as "null".
    fn make_threads(n: usize) -> alloc::vec::Vec<ThreadContext> {
        let mut v = alloc::vec::Vec::new();
        for _ in 0..n {
            v.push(ThreadContext::zeroed());
        }
        v
    }

    #[test]
    fn test_readylist_empty() {
        let mut rl = ReadyList::new();
        assert!(!rl.any_valid());
        assert_eq!(rl.best_prio(), MAX_PRIOS);
    }

    #[test]
    fn test_readylist_single_thread() {
        let mut rl = ReadyList::new();
        let mut threads = make_threads(4);

        threads[1].prio = 10;
        rl.append(&mut threads, 1);

        assert!(rl.any_valid());
        assert_eq!(rl.best_prio(), 10);
        assert!(rl.prio_valid(10));
        assert!(!rl.prio_valid(9));
        assert_eq!(threads[1].status, ThreadStatus::Ready as u8);
    }

    #[test]
    fn test_readylist_priority_ordering() {
        let mut rl = ReadyList::new();
        let mut threads = make_threads(8);

        threads[1].prio = 50;
        threads[2].prio = 10;
        threads[3].prio = 30;

        rl.append(&mut threads, 1);
        rl.append(&mut threads, 2);
        rl.append(&mut threads, 3);

        // Best priority should be 10 (numerically lowest)
        assert_eq!(rl.best_prio(), 10);

        // getbest should return thread at prio 10 first
        let best = rl.getbest(&mut threads);
        assert_eq!(best, 2);

        // Next best should be prio 30
        assert_eq!(rl.best_prio(), 30);
        let best = rl.getbest(&mut threads);
        assert_eq!(best, 3);

        // Then prio 50
        assert_eq!(rl.best_prio(), 50);
        let best = rl.getbest(&mut threads);
        assert_eq!(best, 1);

        // Now empty
        assert!(!rl.any_valid());
        assert_eq!(rl.getbest(&mut threads), IDX_NONE);
    }

    #[test]
    fn test_readylist_same_priority_fifo() {
        let mut rl = ReadyList::new();
        let mut threads = make_threads(8);

        // All at same priority
        threads[1].prio = 5;
        threads[2].prio = 5;
        threads[3].prio = 5;

        rl.append(&mut threads, 1);
        rl.append(&mut threads, 2);
        rl.append(&mut threads, 3);

        // Should come out in FIFO order
        assert_eq!(rl.getbest(&mut threads), 1);
        assert_eq!(rl.getbest(&mut threads), 2);
        assert_eq!(rl.getbest(&mut threads), 3);
        assert_eq!(rl.getbest(&mut threads), IDX_NONE);
    }

    #[test]
    fn test_readylist_insert_front() {
        let mut rl = ReadyList::new();
        let mut threads = make_threads(8);

        threads[1].prio = 5;
        threads[2].prio = 5;
        threads[3].prio = 5;

        rl.append(&mut threads, 1);
        rl.append(&mut threads, 2);
        // Insert at front (before thread 1)
        rl.insert(&mut threads, 3);

        // Thread 3 should come out first
        assert_eq!(rl.getbest(&mut threads), 3);
        assert_eq!(rl.getbest(&mut threads), 1);
        assert_eq!(rl.getbest(&mut threads), 2);
    }

    #[test]
    fn test_readylist_remove_middle() {
        let mut rl = ReadyList::new();
        let mut threads = make_threads(8);

        threads[1].prio = 5;
        threads[2].prio = 5;
        threads[3].prio = 5;

        rl.append(&mut threads, 1);
        rl.append(&mut threads, 2);
        rl.append(&mut threads, 3);

        // Remove middle
        rl.remove(&mut threads, 2);

        assert_eq!(rl.getbest(&mut threads), 1);
        assert_eq!(rl.getbest(&mut threads), 3);
        assert_eq!(rl.getbest(&mut threads), IDX_NONE);
    }

    #[test]
    fn test_readylist_remove_head() {
        let mut rl = ReadyList::new();
        let mut threads = make_threads(8);

        threads[1].prio = 5;
        threads[2].prio = 5;

        rl.append(&mut threads, 1);
        rl.append(&mut threads, 2);

        // Remove head
        rl.remove(&mut threads, 1);

        assert_eq!(rl.getbest(&mut threads), 2);
        assert_eq!(rl.getbest(&mut threads), IDX_NONE);
    }

    #[test]
    fn test_readylist_remove_last() {
        let mut rl = ReadyList::new();
        let mut threads = make_threads(4);

        threads[1].prio = 5;
        rl.append(&mut threads, 1);
        rl.remove(&mut threads, 1);

        assert!(!rl.any_valid());
        assert!(!rl.prio_valid(5));
    }

    #[test]
    fn test_readylist_high_priorities() {
        let mut rl = ReadyList::new();
        let mut threads = make_threads(4);

        threads[1].prio = 200;
        threads[2].prio = 255;

        rl.append(&mut threads, 1);
        rl.append(&mut threads, 2);

        assert_eq!(rl.best_prio(), 200);
        assert_eq!(rl.getbest(&mut threads), 1);
        assert_eq!(rl.best_prio(), 255);
        assert_eq!(rl.getbest(&mut threads), 2);
    }
}
