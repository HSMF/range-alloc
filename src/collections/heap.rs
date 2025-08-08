use core::{
    fmt,
    marker::PhantomData,
    ptr::{self, NonNull, addr_eq},
};

type Link<T> = Option<NonNull<Node<T>>>;

const HEAP_INVARIANT: &str = "invariant: we have a full heap";

struct Node<T> {
    value: T,
    left: Link<T>,
    right: Link<T>,
    parent: Option<NonNull<Node<T>>>,
}

fn as_ptr<T>(l: Link<T>) -> *const Node<T> {
    match l {
        Some(p) => p.as_ptr(),
        None => ptr::null(),
    }
}

impl<T> Node<T> {
    fn new_boxed(value: T) -> NonNull<Self> {
        let s = Self {
            value,
            left: None,
            right: None,
            parent: None,
        };
        let s = Box::into_raw(Box::new(s));
        NonNull::new(s).unwrap()
    }
}

pub struct Heap<T> {
    root: Link<T>,
    len: usize,

    _d: PhantomData<T>,
}

macro_rules! ref_or_mut {
    (mut $e:expr) => {
        &mut $e
    };
    (const $e:expr) => {
        &$e
    };
}

macro_rules! cast_ref_or_mut {
    (mut $e:expr) => {
        unsafe { &mut *$e.as_ptr() }
    };
    (const $e:expr) => {
        unsafe { $e.as_ref() }
    };
}

macro_rules! get_node_at {
    ($root:expr, $pos:expr, $as:tt) => {{
        let root = ref_or_mut!($as *{
            $root?
        });
        let pos = $pos + 1;
        let mut cur = cast_ref_or_mut!($as root);
        for bit in (0..pos.ilog2()).rev() {
            let next = if pos & (1 << bit) == 0 {
                ref_or_mut!($as cur.left)
            } else {
                ref_or_mut!($as cur.right)
            };
            let Some(next) = next else {
                return None;
            };
            let next = cast_ref_or_mut!($as next);
            cur = ref_or_mut!($as *next);
        }
        Some(NonNull::from(cur))
    }};
}

impl<T> Heap<T> {
    pub fn new() -> Self {
        Self {
            root: None,
            len: 0,
            _d: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn insert_at_bottom(&mut self, val: T) -> NonNull<Node<T>> {
        if self.root.is_none() {
            self.len += 1;
            self.root = Some(Node::new_boxed(val));
            return self.root.expect("we just put it there");
        }

        let loc = self.len + 1;
        self.len += 1;
        let cur = self.get_node_at_mut(loc / 2 - 1);
        let mut cur = cur.expect(HEAP_INVARIANT);

        let mut new = Node::new_boxed(val);

        let ret = if loc & 1 == 0 {
            let cur = unsafe { cur.as_mut() };
            cur.left = Some(new);
            cur.left.expect("we just put it there")
        } else {
            let cur = unsafe { cur.as_mut() };
            cur.right = Some(new);
            cur.right.expect("we just put it there")
        };
        unsafe { new.as_mut().parent = Some(cur) };
        ret
    }

    fn get_node_at_mut(&mut self, pos: usize) -> Option<NonNull<Node<T>>> {
        let root = self.root?;
        let pos = pos + 1;
        let mut cur = root;
        for bit in (0..pos.ilog2()).rev() {
            let next = if pos & (1 << bit) == 0 {
                let mut cur = unsafe { cur.as_mut() };
                ref_or_mut!(mut cur.left)
            } else {
                let mut cur = unsafe { cur.as_mut() };
                ref_or_mut!(mut cur.right)
            };
            let Some(next) = next else {
                return None;
            };
            cur = *next;
        }
        Some(cur)
    }

    fn remove_leaf(&mut self, last: *mut Node<T>) -> Box<Node<T>> {
        if let Some(mut parent) = unsafe { (*last).parent } {
            let parent = unsafe { parent.as_mut() };
            if addr_eq(as_ptr(parent.left), last) {
                parent.left = None;
            } else {
                assert!(addr_eq(as_ptr(parent.right), last));
                parent.right = None;
            }
        } else {
            assert_eq!(self.len, 1);
            self.root = None;
        };
        self.len -= 1;

        unsafe { Box::from_raw(last) }
    }
}

impl<T: Ord> Heap<T> {
    pub fn insert(&mut self, v: T) {
        let mut new = self.insert_at_bottom(v);
        let mut new = unsafe { new.as_mut() };

        loop {
            let Some(mut parent) = new.parent else {
                return;
            };

            let parent = unsafe { parent.as_mut() };
            if parent.value > new.value {
                return;
            }

            core::mem::swap(&mut parent.value, &mut new.value);
            new = parent;
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        let mut node = self.root?;

        let mut last = self.get_node_at_mut(self.len - 1).expect(HEAP_INVARIANT);
        {
            let last = unsafe { last.as_ref() };
            assert!(last.left.is_none());
            assert!(last.right.is_none());
        }

        if addr_eq(last.as_ptr(), node.as_ptr()) {
            // removing a leaf (in this case root) is cheap
            let last = last.as_ptr(); // ew
            let last = self.remove_leaf(last);

            return Some(last.value);
        }

        // from now on, node and last are definitely different
        // so it should be safe to construct a mutable reference

        {
            let node = unsafe { node.as_mut() };
            let mut last = unsafe { last.as_mut() };
            core::mem::swap(&mut node.value, &mut last.value);
        }

        let last = last.as_ptr();
        let last = self.remove_leaf(last);

        if self.root.is_none() {
            return Some(last.value);
        }

        self.heapify_down(node);

        Some(last.value)
    }

    // fn get_node_at(&self, pos: usize) -> Option<&Node<T>> {
    //     get_node_at!(self.root.as_ref(), pos, const)
    // }

    fn heapify_down(&mut self, node: NonNull<Node<T>>) {
        let mut cur = node;
        loop {
            let curr = unsafe { cur.as_ref() };
            macro_rules! one_child_none {
                ($cur:expr, $child:expr) => {
                    let cur = unsafe { $cur.as_mut() };
                    let child = unsafe { $child.as_mut() };
                    if child.value < cur.value {
                        return;
                    }

                    core::mem::swap(&mut child.value, &mut cur.value);
                    $cur = $child;
                };
            }
            let Some(mut left) = curr.left else {
                let Some(mut child) = curr.right else {
                    return;
                };
                one_child_none!(cur, child);
                continue;
            };
            let Some(mut right) = curr.right else {
                one_child_none!(cur, left);
                continue;
            };

            let right_r = unsafe { right.as_ref() };
            let left_r = unsafe { left.as_ref() };

            if right_r.value < curr.value && left_r.value < curr.value {
                return;
            }

            let mut max_child = if right_r.value > left_r.value {
                right
            } else {
                left
            };

            {
                let max = unsafe { max_child.as_mut() };
                let curr = unsafe { cur.as_mut() };
                core::mem::swap(&mut curr.value, &mut max.value);
            };
            cur = max_child;
        }
    }
}

impl<T: Ord> Default for Heap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Node<T> {
    fn get_leftmost(&mut self) -> Link<T> {
        let mut left = NonNull::from(self);
        loop {
            let l = unsafe { left.as_ref() };
            if let Some(new) = l.left {
                left = new;
            } else {
                return Some(left);
            }
        }
    }

    fn next_node_mut(&mut self) -> Link<T> {
        if let Some(mut right) = self.right {
            unsafe { right.as_mut().get_leftmost() }
        } else {
            self.parent
        }
    }
}

impl<T> Drop for Heap<T> {
    fn drop(&mut self) {
        let Some(mut root) = self.root else { return };
        let Some(left) = (unsafe { root.as_mut().get_leftmost() }) else {
            return;
        };

        let mut cur = Some(left);
        while let Some(mut node) = cur {
            cur = unsafe { node.as_mut().next_node_mut() };
            if let Some(mut cur) = cur {
                unsafe { cur.as_mut().parent = node.as_ref().parent };
            }

            let _ = unsafe { Box::from_raw(node.as_ptr()) };
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Heap<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn inner<T: fmt::Debug>(
            node: &Node<T>,
            nextid: &mut u64,
            f: &mut std::fmt::Formatter<'_>,
        ) -> Result<u64, fmt::Error> {
            let me = *nextid;
            *nextid += 1;

            writeln!(
                f,
                r#"node{me} [label="{{{{{:?}}}|{{<l>|<r>}}}}"]"#,
                node.value
            )?;

            if let Some(left) = node.left {
                let left = unsafe { left.as_ref() };
                let i = inner(left, nextid, f)?;
                writeln!(f, "node{me}:l -> node{i};")?;
            }

            if let Some(right) = node.right {
                let right = unsafe { right.as_ref() };
                let i = inner(right, nextid, f)?;
                writeln!(f, "node{me}:r -> node{i};")?;
            }

            Ok(me)
        }

        writeln!(f, "digraph {{")?;
        writeln!(f, "node[shape=record,fontname=monospace];")?;
        if let Some(root) = self.root {
            inner(unsafe { root.as_ref() }, &mut 0, f)?;
        }
        writeln!(f, "}}")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::collections::heap::Heap;

    #[test]
    fn it_works() {
        let mut h = Heap::new();
        h.insert(1);
        h.insert(2);
        h.insert(0);

        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(0));
        assert_eq!(h.pop(), None);
    }

    #[test]
    fn dropping() {
        let mut h = Heap::new();
        h.insert(1);
        drop(h);
    }
}
