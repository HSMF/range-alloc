use core::{
    fmt,
    marker::PhantomData,
    ptr::{self, NonNull, addr_eq},
};

type Link<T> = Option<NonNull<Node<T>>>;

const HEAP_INVARIANT: &str = "invariant: we have a full heap";

#[derive(Debug)]
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

impl<T: fmt::Debug> Heap<T> {
    fn swap_parent_child(&mut self, parent: NonNull<Node<T>>, child: NonNull<Node<T>>) {
        let mut parentp = parent;
        let mut childp = child;
        let (parent, child) = unsafe { (parentp.as_mut(), childp.as_mut()) };
        let parent_is_left_child = parent.parent.is_some_and(|grandparent| unsafe {
            addr_eq(as_ptr(grandparent.as_ref().left), parent)
        });
        let child_is_left_child = addr_eq(as_ptr(parent.left), child);

        use core::mem::swap;

        child.parent = parent.parent;
        parent.parent = Some(childp);

        if child_is_left_child {
            swap(&mut child.right, &mut parent.right);
            parent.left = child.left;
            child.left = Some(parentp);
            if let Some(mut right) = child.right {
                unsafe { right.as_mut().parent = Some(childp) };
            }
        } else {
            swap(&mut child.left, &mut parent.left);
            parent.right = child.right;
            child.right = Some(parentp);
            if let Some(mut left) = child.left {
                unsafe { left.as_mut().parent = Some(childp) };
            }
        }

        if let Some(mut left) = parent.left {
            unsafe { left.as_mut().parent = Some(parentp) };
        }

        if let Some(mut right) = parent.right {
            unsafe { right.as_mut().parent = Some(parentp) };
        }

        if let Some(mut grandparent) = child.parent {
            let grandparent = unsafe { grandparent.as_mut() };
            if parent_is_left_child {
                grandparent.left = Some(childp);
            } else {
                grandparent.right = Some(childp);
            }
        }
    }

    fn swap(&mut self, a: NonNull<Node<T>>, b: NonNull<Node<T>>) {
        let mut ap = a;
        let mut bp = b;
        let (a, b) = unsafe { (ap.as_ref(), bp.as_ref()) };
        if addr_eq(as_ptr(a.parent), b) {
            let is_root = b.parent.is_none();
            self.swap_parent_child(bp, ap);
            if is_root {
                self.root = Some(ap);
            }
            return;
        }
        if addr_eq(as_ptr(b.parent), a) {
            let is_root = a.parent.is_none();
            self.swap_parent_child(ap, bp);
            if is_root {
                self.root = Some(bp);
            }
            return;
        }

        let a_is_left_child = a
            .parent
            .is_some_and(|parent| unsafe { addr_eq(as_ptr(parent.as_ref().left), a) });
        let b_is_left_child = b
            .parent
            .is_some_and(|parent| unsafe { addr_eq(as_ptr(parent.as_ref().left), b) });

        use core::mem::swap;

        let (a, b) = unsafe { (ap.as_mut(), bp.as_mut()) };
        swap(&mut a.left, &mut b.left);
        swap(&mut a.right, &mut b.right);
        swap(&mut a.parent, &mut b.parent);

        if let Some(mut parent) = b.parent {
            unsafe {
                if a_is_left_child {
                    parent.as_mut().left = Some(bp)
                } else {
                    parent.as_mut().right = Some(bp)
                }
            }
        }

        if let Some(mut parent) = a.parent {
            unsafe {
                if b_is_left_child {
                    parent.as_mut().left = Some(ap)
                } else {
                    parent.as_mut().right = Some(ap)
                }
            }
        }

        macro_rules! link_child {
            ($el:expr, $p:expr, $field:ident) => {
                if let Some(mut child) = $el.$field {
                    unsafe { child.as_mut().parent = Some($p) };
                }
            };
        }

        link_child!(a, ap, left);
        link_child!(a, ap, right);
        link_child!(b, bp, left);
        link_child!(b, bp, right);

        if a.parent.is_none() {
            self.root = NonNull::new(a);
        } else if b.parent.is_none() {
            self.root = NonNull::new(b);
        }
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

impl<T: fmt::Debug> Heap<T> {
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

    fn remove_leaf(&mut self, last: NonNull<Node<T>>) {
        if let Some(mut parent) = unsafe { last.as_ref().parent } {
            let parent = unsafe { parent.as_mut() };
            if addr_eq(as_ptr(parent.left), last.as_ptr()) {
                parent.left = None;
            } else {
                assert!(addr_eq(as_ptr(parent.right), last.as_ptr()));
                parent.right = None;
            }
        } else {
            assert_eq!(self.len, 1);
            self.root = None;
        };
        self.len -= 1;
    }

    fn iter_ptr(&mut self) -> HeapIter<'_, T> {
        HeapIter { heap: self, i: 0 }
    }
}

impl<T: Ord + fmt::Debug> Heap<T> {
    pub fn insert(&mut self, v: T) {
        let mut new = self.insert_at_bottom(v);
        // let mut new = unsafe { new.as_mut() };

        loop {
            let (newp, new) = unsafe { (new, new.as_ref()) };
            let Some(mut parentp) = new.parent else {
                return;
            };

            let parent = unsafe { parentp.as_mut() };

            if parent.value > new.value {
                return;
            }

            self.swap(parentp, newp);
            // core::mem::swap(&mut parent.value, &mut new.value);

            // new = parent;
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        let mut node = self.root?;

        let mut replacement = self.get_node_at_mut(self.len - 1).expect(HEAP_INVARIANT);
        {
            let replacement = unsafe { replacement.as_ref() };
            assert!(replacement.left.is_none());
            assert!(replacement.right.is_none());
        }

        if addr_eq(replacement.as_ptr(), node.as_ptr()) {
            // removing a leaf (in this case root) is cheap
            self.remove_leaf(replacement);

            let last = unsafe { Box::from_raw(node.as_ptr()) };
            return Some(last.value);
        }

        // from now on, node and last are definitely different
        // so it should be safe to construct a mutable reference

        self.swap(node, replacement);

        self.remove_leaf(node);

        if self.root.is_none() {
            let last = unsafe { Box::from_raw(node.as_ptr()) };
            return Some(last.value);
        }

        self.heapify_down(replacement);

        let last = unsafe { Box::from_raw(node.as_ptr()) };
        Some(last.value)
    }

    // fn get_node_at(&self, pos: usize) -> Option<&Node<T>> {
    //     get_node_at!(self.root.as_ref(), pos, const)
    // }

    fn heapify_down(&mut self, node: NonNull<Node<T>>) {
        let mut cur = node;
        loop {
            let curr = unsafe { cur.as_ref() };
            let (mut left, mut right) = match (curr.left, curr.right) {
                (None, None) => return,
                (None, Some(mut child)) | (Some(mut child), None) => {
                    let cur_r = unsafe { cur.as_ref() };
                    let child_r = unsafe { child.as_ref() };
                    if child_r.value < cur_r.value {
                        return;
                    }
                    self.swap(child, cur);
                    continue;
                }
                (Some(left), Some(right)) => (left, right),
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

            self.swap(cur, max_child);
        }
    }
}

impl<T: Ord + fmt::Debug> Default for Heap<T> {
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
            return unsafe { right.as_mut().get_leftmost() };
        }
        let mut cur = self;
        loop {
            let mut next = cur.parent?;
            if addr_eq(as_ptr(unsafe { next.as_mut() }.left), cur) {
                return Some(next);
            }
        }
    }
}

impl<T> Drop for Heap<T> {
    fn drop(&mut self) {
        // uses O(n) memory... can we avoid this?
        let Some(mut root) = self.root else { return };

        fn free<T>(node: NonNull<Node<T>>) {
            if let Some(left) = unsafe { node.as_ref().left } {
                free(left)
            }
            if let Some(right) = unsafe { node.as_ref().right } {
                free(right)
            }
            let _ = unsafe { Box::from_raw(node.as_ptr()) };
        }

        if let Some(root) = self.root {
            free(root)
        }
        // let Some(left) = (unsafe { root.as_mut().get_leftmost() }) else {
        //     return;
        // };
        //
        // let mut cur = Some(left);
        //
        // while let Some(mut node) = cur {
        //     let next = unsafe { node.as_mut().next_node_mut() };
        //     let _ = unsafe { Box::from_raw(node.as_ptr()) };
        //     println!("done with {cur:?}, onto {next:?}");
        //     cur = next;
        // }
    }
}

impl<T: fmt::Debug> fmt::Debug for Heap<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn inner<T: fmt::Debug>(
            node: &Node<T>,
            nextid: &mut u64,
            lim: u64,
            f: &mut std::fmt::Formatter<'_>,
        ) -> Result<u64, fmt::Error> {
            let me = *nextid;
            let me = node as *const _ as usize as u64;
            *nextid += 1;
            if lim == 0 {
                writeln!(f, "// warning! recursion depth exceeded")?;
                return Ok(me);
            }

            writeln!(
                f,
                r#"node{me:x} [label="{{{{{:?}}}|{{<l>|<r>}}}}"]"#,
                node.value
            )?;

            if let Some(parent) = node.parent {
                writeln!(
                    f,
                    "node{me:x} -> node{:x} [color=red]",
                    parent.as_ptr() as usize as u64
                );
            }

            if let Some(left) = node.left {
                let left = unsafe { left.as_ref() };
                let i = inner(left, nextid, lim - 1, f)?;
                writeln!(f, "node{me:x}:l -> node{i:x};")?;
            }

            if let Some(right) = node.right {
                let right = unsafe { right.as_ref() };
                let i = inner(right, nextid, lim - 1, f)?;
                writeln!(f, "node{me:x}:r -> node{i:x};")?;
            }

            Ok(me)
        }

        writeln!(f, "digraph {{")?;
        writeln!(f, "node[shape=record,fontname=monospace];")?;
        if let Some(root) = self.root {
            inner(unsafe { root.as_ref() }, &mut 0, 20, f)?;
        }
        writeln!(f, "}}")?;

        Ok(())
    }
}

struct HeapIter<'a, T> {
    heap: &'a mut Heap<T>,
    i: usize,
}

impl<'a, T: fmt::Debug> Iterator for HeapIter<'a, T> {
    type Item = NonNull<Node<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        let n = self.heap.get_node_at_mut(self.i);
        self.i += 1;
        n
    }
}

#[cfg(test)]
mod tests {
    use crate::collections::heap::Heap;

    #[test]
    fn new_heap_is_empty() {
        let mut heap: Heap<i32> = Heap::new();
        assert_eq!(heap.len(), 0);
        assert_eq!(heap.pop(), None);
    }

    #[test]
    fn insert_increases_len() {
        let mut heap = Heap::new();
        heap.insert(10);
        assert_eq!(heap.len(), 1);
        heap.insert(5);
        assert_eq!(heap.len(), 2);
    }

    #[test]
    fn pop_returns_maximum() {
        let mut heap = Heap::new();
        heap.insert(1);
        heap.insert(10);
        heap.insert(5);

        assert_eq!(heap.pop(), Some(10));
        assert_eq!(heap.pop(), Some(5));
        assert_eq!(heap.pop(), Some(1));
        assert_eq!(heap.pop(), None);
    }

    #[test]
    fn pop_reduces_len() {
        let mut heap = Heap::new();
        heap.insert(3);
        heap.insert(7);

        heap.pop();
        assert_eq!(heap.len(), 1);
        heap.pop();
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn handles_duplicate_values() {
        let mut heap = Heap::new();
        heap.insert(5);
        heap.insert(5);
        heap.insert(5);

        assert_eq!(heap.pop(), Some(5));
        assert_eq!(heap.pop(), Some(5));
        assert_eq!(heap.pop(), Some(5));
        assert_eq!(heap.pop(), None);
    }

    #[test]
    fn works_with_negative_numbers() {
        let mut heap = Heap::new();
        heap.insert(-10);
        heap.insert(-1);
        heap.insert(-5);

        assert_eq!(heap.pop(), Some(-1));
        assert_eq!(heap.pop(), Some(-5));
        assert_eq!(heap.pop(), Some(-10));
    }

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

    use proptest::prelude::*;
    proptest! {
        #[cfg_attr(miri, ignore)]
        #[test]
        fn heap_pops_in_sorted_order(xs in proptest::collection::vec(any::<i32>(), 0..100)) {
            let mut heap = Heap::new();
            for x in &xs {
                heap.insert(*x);
            }

            let mut elems = Vec::with_capacity(xs.len());
            while let Some(v) = heap.pop() {
                elems.push(v);
            }

            // Check length matches
            prop_assert_eq!(elems.len(), xs.len());

            // Ensure it's sorted in non-increasing order
            prop_assert!(elems.windows(2).all(|w| w[0] >= w[1]));
        }
    }

    #[test]
    fn iter() {
        let mut heap = Heap::new();

        heap.insert(1);
        heap.insert(2);
        heap.insert(0);

        let mut it = heap.iter_ptr();
        unsafe { assert_eq!(it.next().unwrap().as_ref().value, 2) }
        unsafe { assert_eq!(it.next().unwrap().as_ref().value, 1) }
        unsafe { assert_eq!(it.next().unwrap().as_ref().value, 0) }
    }
}
