use std::ptr::NonNull;

use tinyvec::ArrayVec;

use crate::{Error, Result};

/// maximum number of buckets per node
const B: usize = 6;

#[derive(Default)]
struct Bucket<Tag> {
    tag: Tag,
    child: Option<NonNull<Node<Tag>>>,
}

struct Node<Tag> {
    buckets: ArrayVec<[Bucket<Tag>; B]>,
}
pub struct RangeAllocator<Tag> {
    root: Node<Tag>,
}

impl<Tag: Default> RangeAllocator<Tag> {
    pub fn new() -> Self {
        RangeAllocator {
            root: Node {
                buckets: ArrayVec::new(),
            },
        }
    }

    /// adds a range to the allocator from which the allocator may pick
    pub fn add_range(&mut self, base: usize, size: usize, range_tag: Tag) -> Result<()> {
        Err(Error::unimplemented())
    }

    /// allocates a range. The range will not be handed out again until it has been freed
    pub fn alloc(&mut self, min_size: usize, alignment: usize) -> Result<(Tag, usize)> {
        Err(Error::unimplemented())
    }
    /// allocates a range at the given base address. Fails if that address is already allocated.
    pub fn alloc_fixed(&mut self, base: usize, size: usize) -> Result<(Tag, usize)> {
        Err(Error::unimplemented())
    }

    /// frees a previously handed out range
    pub fn free(&mut self, base: usize) -> Result<Tag> {
        Err(Error::unimplemented())
    }
}

impl<Tag: Default> Default for RangeAllocator<Tag> {
    fn default() -> Self {
        Self::new()
    }
}
