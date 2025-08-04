use std::{ops::Range, ptr::NonNull};

use crate::{Error, Result, round_up};

pub const BASE_PAGE_SIZE: usize = 4096;

struct Node<Tag> {
    tag: Tag,
    base: usize,
    size: usize,
    next: Option<NonNull<Node<Tag>>>,
    prev: Option<NonNull<Node<Tag>>>,
}

fn overlaps<I: PartialOrd>(a: Range<I>, b: Range<I>) -> bool {
    a.contains(&b.start) || b.contains(&a.start)
}

impl<T> Node<T> {
    fn range(&self) -> Range<usize> {
        self.base..self.base + self.size
    }

    fn unlink(&mut self) -> Option<Option<NonNull<Self>>> {
        let mut head = None;
        unsafe {
            if let Some(mut next) = self.next {
                next.as_mut().prev = self.prev;
            }

            if let Some(mut prev) = self.prev {
                prev.as_mut().next = self.next;
            } else {
                // no prev ==> we are head
                head = Some(self.next);
            }
        }
        self.next = None;
        self.prev = None;
        head
    }
}

struct NodeIterMut<'a, T> {
    node: Option<&'a mut Node<T>>,
}

impl<'a, T> Iterator for NodeIterMut<'a, T> {
    type Item = &'a mut Node<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.node.take() {
            self.node = node.next.map(|mut x| unsafe { x.as_mut() });
            Some(node)
        } else {
            None
        }
    }
}

struct NodeIter<'a, T> {
    node: Option<&'a Node<T>>,
}

impl<'a, T> Iterator for NodeIter<'a, T> {
    type Item = &'a Node<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.node.take() {
            self.node = node.next.map(|x| unsafe { x.as_ref() });
            Some(node)
        } else {
            None
        }
    }
}

pub struct RangeAllocator<Tag> {
    head: Option<NonNull<Node<Tag>>>,
    mem_regions: Option<NonNull<Node<Tag>>>,
}

impl<T> RangeAllocator<T> {
    pub fn new() -> Self {
        RangeAllocator {
            head: None,
            mem_regions: None,
        }
    }
}

macro_rules! insert_to_list {
    ($this:expr, $list:ident,
            $base:expr, $size:expr,
            $tag: expr) => {{
        let new_first = $this.pin(Node {
            tag: $tag,
            base: $base,
            size: $size,
            next: $this.$list,
            prev: None,
        });
        if let Some(mut old_first) = $this.$list {
            unsafe { old_first.as_mut().prev = Some(new_first) };
        }
        $this.$list = Some(new_first);
    }};
}

macro_rules! remove_from_list {
    ($this:expr, $list:ident, $node:expr) => {{
        let node = $node;
        let new_head = node.unlink();
        let node = NonNull::from(node);
        $this.release(node);
        if let Some(head) = new_head {
            $this.$list = head;
        }
    }};
}

impl<Tag> RangeAllocator<Tag> {
    fn iter_mut(&mut self) -> NodeIterMut<'_, Tag> {
        NodeIterMut {
            node: self.head.map(|mut x| unsafe { x.as_mut() }),
        }
    }

    fn iter(&self) -> NodeIter<'_, Tag> {
        NodeIter {
            node: self.head.map(|x| unsafe { x.as_ref() }),
        }
    }

    fn parent_iter(&self) -> NodeIter<'_, Tag> {
        NodeIter {
            node: self.mem_regions.map(|x| unsafe { x.as_ref() }),
        }
    }

    #[track_caller]
    fn pin(&self, n: Node<Tag>) -> NonNull<Node<Tag>> {
        unsafe { NonNull::new_unchecked(Box::into_raw(Box::new(n))) }
    }

    #[track_caller]
    fn release(&self, n: NonNull<Node<Tag>>) {
        unsafe { drop(Box::from_raw(n.as_ptr())) };
    }
}

impl<Tag> RangeAllocator<Tag>
where
    Tag: Default + Clone,
{
    /// adds a range to the allocator from which the allocator may pick
    pub fn add_range(&mut self, base: usize, size: usize, range_tag: Tag) -> Result<()> {
        assert!(size > 0);
        eprintln!("add_range {base}:{size}");
        if self
            .iter_mut()
            .any(|x| overlaps(x.range(), base..base + size))
        {
            return Err(Error::cause("overlapping range"));
        }

        insert_to_list!(self, head, base, size, range_tag.clone());
        insert_to_list!(self, mem_regions, base, size, range_tag);

        Ok(())
    }

    /// allocates a range. The range will not be handed out again until it has been freed
    pub fn alloc(&mut self, min_size: usize, alignment: usize) -> Result<(Tag, usize)> {
        eprintln!(
            "allocate: {min_size} {alignment} currently have space: {}",
            self.space()
        );
        if !alignment.is_power_of_two() {
            return Err(Error::cause("not power of two"));
        }
        let min_size = round_up!(min_size, BASE_PAGE_SIZE);

        let mut any_can_fit = false;

        let candidate = self.iter_mut().find(|node| {
            if min_size > node.size {
                return false;
            }
            // this node has enough space for the request, but does it satisfy the constraints?
            any_can_fit = true;

            let aligned = round_up!(node.base, alignment);
            let spill = aligned - node.base;

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

        let Some(candidate) = candidate else {
            if any_can_fit {
                return Err(Error::cause("has space but overconstrained"));
            } else {
                return Err(Error::cause("no space"));
            }
        };

        let free_start = candidate.base;
        let after_free = candidate.base + candidate.size;

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
                remove_from_list!(self, head, candidate);

                (free_start, after_free - free_start)
            }
            (None, Some(after)) => {
                candidate.base = after.0;
                candidate.size = after.1 - after.0;
                (free_start, after_allocated - free_start)
            }
            (Some(before), None) => {
                candidate.base = before.0;
                candidate.size = before.1 - before.0;
                (allocated_start, after_free - allocated_start)
            }
            (Some(before), Some(after)) => {
                candidate.base = before.0;
                candidate.size = before.1 - before.0;
                let tag = candidate.tag.clone();
                let mut candidate = NonNull::from(candidate);

                let new = self.pin(Node {
                    tag,
                    base: after.0,
                    size: after.1 - after.0,
                    next: Some(candidate),
                    prev: None,
                });

                unsafe { candidate.as_mut().prev = Some(new) };

                (allocated_start, after_allocated - allocated_start)
            }
        };

        Ok((tag, addr))
    }
    /// allocates a range at the given base address. Fails if that address is already allocated.
    pub fn alloc_fixed(&mut self, base: usize, size: usize) -> Result<(Tag, usize)> {
        Err(Error::unimplemented())
    }

    /// frees a previously handed out range
    pub fn free(&mut self, base: usize, size: usize) -> Result<()> {
        let parent_region = self
            .parent_iter()
            .find(|parent| parent.range().contains(&base));

        let Some(parent_region) = parent_region else {
            return Err(Error::cause("not allocated by this allocator"));
        };

        fn to_non_null<T>(x: Option<&mut T>) -> Option<NonNull<T>> {
            x.map(NonNull::from)
        }

        let parent_tag = parent_region.tag.clone();

        let mut adjacent_before = None;
        let mut adjacent_after = None;
        for node in self.iter_mut() {
            if node.base + node.size == base {
                adjacent_before = Some(node)
            } else if base + size == node.base {
                adjacent_after = Some(node)
            }
        }

        match (adjacent_before, adjacent_after) {
            (None, None) => {
                insert_to_list!(self, head, base, size, parent_tag)
            }
            (Some(before), None) => {
                before.size += size;
            }
            (None, Some(after)) => {
                after.size += size;
                after.base -= size;
            }
            (Some(before), Some(after)) => {
                let total_size = before.size + size + after.size;
                before.size = total_size;

                remove_from_list!(self, head, after);
            }
        }

        Ok(())
    }

    pub fn space(&self) -> usize {
        self.iter().map(|x| x.size).sum()
    }
}

impl<Tag> Drop for RangeAllocator<Tag> {
    fn drop(&mut self) {
        while let Some(mut node) = self.head {
            let node = unsafe { node.as_mut() };
            remove_from_list!(self, head, node);
        }

        while let Some(mut node) = self.mem_regions {
            let node = unsafe { node.as_mut() };
            remove_from_list!(self, mem_regions, node);
        }
    }
}

impl<Tag: Default> Default for RangeAllocator<Tag> {
    fn default() -> Self {
        Self::new()
    }
}
