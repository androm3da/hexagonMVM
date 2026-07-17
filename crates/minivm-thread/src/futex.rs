/*
 * Copyright (c) Qualcomm Technologies, Inc. and/or its subsidiaries.
 * SPDX-License-Identifier: BSD-3-Clause-Clear
 */

//! Futex wait/wake (hash table).
//!
//! Futex waiters are stored in a hash table of per-bucket `BinaryHeap`s,
//! ordered by `(priority, sequence)` so the best-priority waiter (lowest
//! priority value, earliest sequence) is always at the head. The hash
//! function uses FNV prime multiplication on the physical address of the
//! futex.
//!
//! `remove_one`/`cancel` need to remove an entry by content (matching
//! futex address) or by thread identity, not just pop the head — since
//! `BinaryHeap` has no such operation, and the futex path is not yet wired
//! to a real syscall (low frequency), these rebuild the bucket via a
//! priority-order drain-and-filter rather than tracking a side table.

use alloc::collections::BinaryHeap;
use core::cmp::Reverse;

/// Number of hash bits.
pub const FUTEX_HASHBITS: u32 = 6;
/// Number of hash buckets.
pub const FUTEX_HASHSIZE: usize = 1 << FUTEX_HASHBITS;
/// FNV hash prime.
pub const FUTEX_PRIME: u32 = 2654435761;

/// Sentinel for empty ring/no thread.
pub const IDX_NONE: u32 = 0;

/// Compute the futex hash value for a physical address (shifted by 2).
///
/// This matches the C `FUTEX_HASHVAL` macro.
pub fn futex_hash(pa_shifted: u64) -> usize {
    let lo = pa_shifted as u32;
    let hi = (pa_shifted >> 32) as u32;
    let hash = lo.wrapping_mul(FUTEX_PRIME);
    let bits = (hash >> (32 - FUTEX_HASHBITS)) & ((1 << FUTEX_HASHBITS) - 1);
    (bits ^ hi) as usize % FUTEX_HASHSIZE
}

/// Heap entry: `(priority, sequence, thread index)`. Lower priority value
/// and lower sequence sort first (best priority, then FIFO).
type Entry = Reverse<(u8, u64, u32)>;

/// A futex hash table with per-bucket priority-ordered waiter heaps.
pub struct FutexTable {
    /// Hash buckets, each a min-heap of waiters ordered by (priority, seq).
    pub buckets: [BinaryHeap<Entry>; FUTEX_HASHSIZE],
    /// Monotonically increasing counter for FIFO ordering within a priority.
    next_seq: u64,
}

impl Default for FutexTable {
    fn default() -> Self {
        Self::new()
    }
}

impl FutexTable {
    pub fn new() -> Self {
        Self {
            buckets: core::array::from_fn(|_| BinaryHeap::new()),
            next_seq: 0,
        }
    }

    /// Add a waiter to the hash bucket for the given futex address.
    ///
    /// `hash`: the hash bucket index
    /// `idx`: the thread index to add
    /// `prio`: the thread's priority
    pub fn add_waiter(&mut self, hash: usize, idx: u32, prio: u8) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.buckets[hash].push(Reverse((prio, seq, idx)));
    }

    /// Remove the first (best-priority) waiter matching `futex_lo` from the
    /// hash bucket.
    ///
    /// Returns the index of the removed waiter, or `IDX_NONE` if not found.
    pub fn remove_one(&mut self, hash: usize, futex_lo: u32, futex_ptrs: &[u32]) -> u32 {
        let bucket = &mut self.buckets[hash];
        let mut pending = alloc::vec::Vec::new();
        let mut found = IDX_NONE;

        while let Some(Reverse((prio, seq, idx))) = bucket.pop() {
            if found == IDX_NONE && futex_ptrs[idx as usize] == futex_lo {
                found = idx;
            } else {
                pending.push(Reverse((prio, seq, idx)));
            }
        }
        bucket.extend(pending);
        found
    }

    /// Remove a specific thread from its hash bucket.
    ///
    /// Used by futex_cancel to remove a blocked thread.
    pub fn cancel(&mut self, hash: usize, idx: u32) {
        let bucket = &mut self.buckets[hash];
        let remaining: BinaryHeap<Entry> = bucket
            .drain()
            .filter(|&Reverse((_, _, entry_idx))| entry_idx != idx)
            .collect();
        *bucket = remaining;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_futex_hash_distribution() {
        // Different addresses should hash to different buckets (mostly)
        let h1 = futex_hash(0x1000);
        let h2 = futex_hash(0x2000);
        let h3 = futex_hash(0x3000);
        assert!(h1 < FUTEX_HASHSIZE);
        assert!(h2 < FUTEX_HASHSIZE);
        assert!(h3 < FUTEX_HASHSIZE);
    }

    #[test]
    fn test_futex_add_and_remove() {
        let mut table = FutexTable::new();
        let futex_ptrs = [0x1000u32, 0x1000, 0x1000, 0x2000, 0, 0, 0, 0];

        let hash = 5; // arbitrary bucket

        // Add waiter 1 and 2 for futex 0x1000
        table.add_waiter(hash, 1, 10);
        table.add_waiter(hash, 2, 10);

        // Remove first matching 0x1000
        let removed = table.remove_one(hash, 0x1000, &futex_ptrs);
        assert_eq!(removed, 1);

        // Remove next matching 0x1000
        let removed = table.remove_one(hash, 0x1000, &futex_ptrs);
        assert_eq!(removed, 2);

        // No more
        let removed = table.remove_one(hash, 0x1000, &futex_ptrs);
        assert_eq!(removed, IDX_NONE);
    }

    #[test]
    fn test_futex_priority_ordering() {
        let mut table = FutexTable::new();
        let futex_ptrs = [0u32, 0x1000, 0x1000, 0x1000, 0, 0, 0, 0];

        let hash = 3;

        // Add in order: prio 20, 10, 30
        table.add_waiter(hash, 1, 20);
        table.add_waiter(hash, 2, 10);
        table.add_waiter(hash, 3, 30);

        // Remove first → should be prio 10 (thread 2)
        let removed = table.remove_one(hash, 0x1000, &futex_ptrs);
        assert_eq!(removed, 2);
    }

    #[test]
    fn test_futex_cancel() {
        let mut table = FutexTable::new();

        let hash = 7;

        table.add_waiter(hash, 1, 10);
        table.add_waiter(hash, 2, 10);

        // Cancel thread 1
        table.cancel(hash, 1);

        // Only thread 2 should remain
        assert_eq!(table.buckets[hash].len(), 1);
        let futex_ptrs = [0u32; 8];
        assert_eq!(table.remove_one(hash, futex_ptrs[2], &futex_ptrs), 2);
    }
}
