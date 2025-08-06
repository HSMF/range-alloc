use core::fmt;
use std::{collections::BTreeMap, ptr::NonNull};

use tinyvec::{Array, ArrayVec, array_vec};

use crate::{Error, RangeAlloc, Result, linear::BASE_PAGE_SIZE, round_up};

/// maximum number of buckets per node
pub(super) const B: usize = 6;

const CAPACITY: usize = B;

#[derive(Debug, Default, PartialEq, Eq)]
struct Entry<Tag> {
    size: usize,
    tag: Tag,
}

type EntryWithBase<'a, Tag> = (&'a usize, &'a Entry<Tag>);

pub struct RangeAllocator<Tag> {
    tree: BTreeMap<usize, Entry<Tag>>,
    regions: BTreeMap<usize, Entry<Tag>>,
    total_space: usize,
    free_space: usize,
}

struct P<'a, Tag>(&'a BTreeMap<usize, Entry<Tag>>);
impl<T> fmt::Debug for P<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut list = f.debug_list();
        for i in self.0 {
            list.entry(&format!("{:x}:{}", i.0, i.1.size));
        }

        list.finish();

        Ok(())
    }
}

impl<T: Default> RangeAllocator<T> {
    pub fn new() -> Self {
        RangeAllocator {
            tree: BTreeMap::new(), // TODO: new_in
            regions: BTreeMap::new(),
            total_space: 0,
            free_space: 0,
        }
    }

    fn before_and_after(
        &self,
        base: usize,
        size: usize,
    ) -> (Option<EntryWithBase<T>>, Option<EntryWithBase<T>>) {
        (
            self.tree.range(..base).next_back(),
            self.tree.range(base + size..).next(),
        )
    }
}

impl<Tag: Default + Clone + fmt::Debug> RangeAlloc for RangeAllocator<Tag> {
    type Tag = Tag;

    /// adds a range to the allocator from which the allocator may pick
    fn add_range(&mut self, base: usize, size: usize, range_tag: Tag) -> Result<()> {
        self.free_space += size;
        self.total_space += size;

        let (before, after) = self.before_and_after(base, 0);
        if let Some(before) = before {
            if before.0 + before.1.size > base {
                return Err(Error::cause("adding overlapping region"));
            }
        }

        if let Some(after) = after {
            if base + size > *after.0 {
                return Err(Error::cause("adding overlapping region"));
            }
        }

        self.tree.insert(
            base,
            Entry {
                size,
                tag: range_tag.clone(),
            },
        );
        self.regions.insert(
            base,
            Entry {
                size,
                tag: range_tag,
            },
        );

        Ok(())
    }

    /// allocates a range. The range will not be handed out again until it has been freed
    fn alloc(&mut self, min_size: usize, alignment: usize) -> Result<(Tag, usize)> {
        if !alignment.is_power_of_two() {
            return Err(Error::cause("not power of two"));
        }
        if !alignment.is_power_of_two() {
            return Err(Error::cause("not power of two"));
        }
        let min_size = round_up!(min_size, BASE_PAGE_SIZE);

        let mut any_can_fit = false;

        let candidate = self
            .tree
            .range_mut(usize::MIN..usize::MAX) // TODO: use address range constraints
            .find(|(base, node)| {
                let base = *base;
                if min_size > node.size {
                    return false;
                }
                // this node has enough space for the request, but does it satisfy the constraints?
                any_can_fit = true;

                let aligned = round_up!(base, alignment);
                let spill = aligned - base;

                if spill > node.size {
                    // aligned base is outside of allocation
                    return false;
                }

                if min_size > node.size - spill {
                    // not enough space in this allocation
                    return false;
                }

                true
            });

        let Some((base, candidate)) = candidate else {
            if any_can_fit {
                return Err(Error::cause("has space but overconstrained"));
            } else {
                return Err(Error::cause("no space"));
            }
        };

        let base = *base;
        let free_start = base;
        let after_free = base + candidate.size;

        let allocated_start = round_up!(free_start, alignment);
        let after_allocated = round_up!(allocated_start + min_size, BASE_PAGE_SIZE);

        fn chunk_between(start: usize, end: usize) -> Option<(usize, usize)> {
            if end - start >= BASE_PAGE_SIZE {
                Some((start, end))
            } else {
                None
            }
        }

        let free_chunk_before = chunk_between(free_start, allocated_start);
        let free_chunk_after = chunk_between(after_allocated, after_free);

        let tag = candidate.tag.clone();
        let (addr, _size) = match (free_chunk_before, free_chunk_after) {
            (None, None) => {
                self.free_space -= candidate.size;
                self.tree.remove(&base);
                (free_start, after_free - free_start)
            }
            (None, Some(after)) => {
                // TODO: this case is way more common than (Some(before), None).
                // We should consider allocating at the end of the range in order to
                // trigger the cheap case more often
                let entry = self
                    .tree
                    .remove(&base)
                    .expect("base is definitely contained in map");
                let new_size = after.1 - after.0;
                self.free_space -= (entry.size - new_size);
                self.tree.insert(
                    after.0,
                    Entry {
                        size: after.1 - after.0,
                        ..entry
                    },
                );
                (free_start, after_allocated - free_start)
            }
            (Some(before), None) => {
                candidate.size = before.1 - before.0;
                (allocated_start, after_free - allocated_start)
            }
            (Some(before), Some(after)) => {
                let before_size = before.1 - before.0;
                let after_size = after.1 - after.0;
                let allocation_size = candidate.size - before_size - after_size;
                candidate.size = before_size;

                let tag = candidate.tag.clone();
                self.tree.insert(
                    after.0,
                    Entry {
                        size: after_size,
                        tag,
                    },
                );

                (allocated_start, after_allocated - allocated_start)
            }
        };

        Ok((tag, addr))
    }

    // /// allocates a range at the given base address. Fails if that address is already allocated.
    // fn alloc_fixed(&mut self, base: usize, size: usize) -> Result<(Tag, usize)> {
    //     Err(Error::unimplemented())
    // }

    /// frees a previously handed out range
    fn free(&mut self, base: usize, size: usize) -> Result<()> {
        let source = self
            .regions
            .range(..=base)
            .next_back()
            .ok_or_else(|| Error::cause("no associated allocation"))?;

        let is_in_source = |base, size: usize| {
            (*source.0..source.0 + source.1.size).contains(&base)
                && (*source.0..=source.0 + source.1.size).contains(&(base + size))
        };

        let (before, after) = self.before_and_after(base, size);

        let before = before.filter(|before| {
            before.0 + before.1.size == base && is_in_source(*before.0, before.1.size)
        });
        let after =
            after.filter(|after| base + size == *after.0 && is_in_source(*after.0, after.1.size));

        match (before, after) {
            (None, None) => {
                self.tree.insert(
                    base,
                    Entry {
                        size,
                        tag: source.1.tag.clone(),
                    },
                );
            }
            (None, Some((&after_base, after))) => {
                let after = self
                    .tree
                    .remove(&after_base)
                    .expect("after is definitely in map");
                self.tree.insert(
                    base,
                    Entry {
                        size: size + after.size,
                        tag: after.tag,
                    },
                );
            }
            (Some((&before_base, before)), None) => {
                let before = self
                    .tree
                    .get_mut(&before_base)
                    .expect("before is definitely in map");
                before.size += size;
            }
            (Some((&before_base, before)), Some((&after_base, after))) => {
                let after = self
                    .tree
                    .remove(&after_base)
                    .expect("after is definitely in map");
                let before = self
                    .tree
                    .get_mut(&before_base)
                    .expect("before is definitely in map");
                before.size += after.size + size;
            }
        }
        self.free_space += size;

        Ok(())
    }

    fn total_space(&self) -> usize {
        self.total_space
    }

    fn space(&self) -> usize {
        self.free_space
    }
}

impl<Tag: Default> Default for RangeAllocator<Tag> {
    fn default() -> Self {
        Self::new()
    }
}
