use core::fmt;
use std::ptr::NonNull;

use tinyvec::{Array, ArrayVec, array_vec};

use crate::{Error, RangeAlloc, Result};

/// maximum number of buckets per node
pub(super) const B: usize = 6;

const CAPACITY: usize = B;

type Link<T> = Option<NonNull<Node<T>>>;

#[derive(Default, PartialEq, Eq)]
struct Entry<Tag> {
    base: usize,
    size: usize,
    tag: Tag,
}

impl<Tag: Eq> PartialOrd for Entry<Tag> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<Tag: Eq> Ord for Entry<Tag> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.base.cmp(&other.base).then(self.size.cmp(&other.size))
    }
}

struct Node<Tag> {
    buckets: ArrayVec<[Entry<Tag>; CAPACITY]>,
    edges: [Option<NonNull<Node<Tag>>>; CAPACITY + 1],
}

impl<Tag: Default + Ord> Node<Tag> {
    fn insert(&mut self, base: usize, size: usize, tag: Tag) {
        if self.buckets.is_empty() {
            self.buckets.push(Entry { base, size, tag });
            return;
        }

        for (i, e) in self.buckets.iter().enumerate() {
            match base.cmp(&e.base) {
                std::cmp::Ordering::Less if i == 0 => {
                    // smaller than first element, insert before
                    if self.buckets.len() < self.buckets.capacity() {
                        self.buckets.insert(0, Entry { base, size, tag });
                        let l = self.edges.len() - 1;
                        self.edges.copy_within(..l, 1);
                        return;
                    }
                    todo!("insert as a child")
                }
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => {
                    todo!("handle replacement, this probably won't happen")
                }
                std::cmp::Ordering::Greater if i == self.buckets.capacity() - 1 => {
                    // there's no more space in this node, insert as a child at
                    todo!("there's no more space in this node, insert as a child")
                }
                std::cmp::Ordering::Greater => {
                    // there's space in this node, insert directly after
                    self.buckets.insert(i + 1, Entry { base, size, tag });
                    let l = self.edges.len() - 1;
                    self.edges.copy_within(i + 1..l, 1);
                    return;
                }
            }
        }
    }
}

impl<T: fmt::Debug + Default> fmt::Debug for Node<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn inner<T: fmt::Debug + Default>(
            node: &Node<T>,
            id: &mut u64,
            f: &mut std::fmt::Formatter,
        ) -> core::result::Result<u64, core::fmt::Error> {
            let me = *id;
            *id += 1;

            write!(f, r#"node{me} [label="{{{{"#)?;

            for (i, v) in node.buckets.iter().enumerate() {
                if i != 0 {
                    write!(f, "|")?;
                }
                write!(f, "0x{:x}:{}:{:?}", v.base, v.size, v.tag)?;
            }

            write!(f, "}}|{{")?;
            for i in 0..=node.buckets.len() {
                if i != 0 {
                    write!(f, "|")?;
                }
                write!(f, "<f{i}>")?;
            }
            writeln!(f, r#"}}}}"];"#)?;

            for (i, child) in node
                .edges
                .iter()
                .take(node.buckets.len() + 1)
                .enumerate()
                .flat_map(|(i, x)| x.map(|x| (i, x)))
            {
                unsafe {
                    let child = child.as_ref();
                    let child_id = inner(child, id, f)?;
                    writeln!(f, "node{me}:f{i} -> node{child_id};")?;
                }
            }

            Ok(me)
        }

        let mut id = 0;
        writeln!(f, "digraph {{")?;
        writeln!(f, "node [shape=record,fontname=monospace];")?;
        inner(self, &mut id, f)?;
        writeln!(f, "}}")?;

        Ok(())
    }
}

pub struct RangeAllocator<Tag> {
    root: Node<Tag>,
    total_space: usize,
    free_space: usize,
}

impl<T: Default> RangeAllocator<T> {
    pub fn new() -> Self {
        RangeAllocator {
            root: Node {
                buckets: ArrayVec::new(),
                edges: [None; CAPACITY + 1],
            },
            total_space: 0,
            free_space: 0,
        }
    }
}

impl<Tag: Default + Ord + fmt::Debug> RangeAlloc for RangeAllocator<Tag> {
    type Tag = Tag;

    /// adds a range to the allocator from which the allocator may pick
    fn add_range(&mut self, base: usize, size: usize, range_tag: Tag) -> Result<()> {
        self.free_space += size;
        self.total_space += size;

        self.root.insert(base, size, range_tag);

        println!("{:?}", self.root);

        Ok(())
    }

    /// allocates a range. The range will not be handed out again until it has been freed
    fn alloc(&mut self, min_size: usize, alignment: usize) -> Result<(Tag, usize)> {
        Err(Error::unimplemented())
    }

    // /// allocates a range at the given base address. Fails if that address is already allocated.
    // fn alloc_fixed(&mut self, base: usize, size: usize) -> Result<(Tag, usize)> {
    //     Err(Error::unimplemented())
    // }

    /// frees a previously handed out range
    fn free(&mut self, base: usize, size: usize) -> Result<()> {
        Err(Error::unimplemented())
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
