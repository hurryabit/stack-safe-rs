#![feature(destructuring_assignment, generators, generator_trait)]
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

mod tree {
    use std::cmp::max;

    #[derive(Clone, Debug)]
    pub struct Tree {
        pub value: i64,
        pub children: Vec<Tree>,
    }

    impl Tree {
        pub fn new(id: i64) -> Self {
            Self {
                value: id,
                children: Vec::new(),
            }
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            if !self.children.is_empty() {
                let mut stack = std::mem::take(&mut self.children);
                while let Some(mut node) = stack.pop() {
                    stack.append(&mut node.children);
                }
            }
        }
    }

    impl Tree {
        pub fn depth_recursive(&self) -> usize {
            let mut max_child_depth = 0;
            for child in &self.children {
                max_child_depth = max(max_child_depth, child.depth_recursive());
            }
            max_child_depth + 1
        }

        pub fn depth_stack_safe(&self) -> usize {
            stack_safe::trampoline(|tree: &Tree| {
                move |_: usize| {
                    let mut max_child_depth = 0;
                    for child in &tree.children {
                        let child_depth = yield child;
                        max_child_depth = max(max_child_depth, child_depth);
                    }
                    max_child_depth + 1
                }
            })(self)
        }

        pub fn depth_manual(&self) -> usize {
            stack_safe::trampoline(manual::DepthGen::init)(self)
        }
    }

    mod manual {
        use super::*;
        use std::ops::{Generator, GeneratorState};

        pub enum DepthGen<'a> {
            Init {
                tree: &'a Tree,
            },
            Call {
                max_child_depth: usize,
                children: std::slice::Iter<'a, Tree>,
            },
            Done,
        }

        static_assertions::const_assert_eq!(std::mem::size_of::<DepthGen>(), 32);

        impl<'a> DepthGen<'a> {
            #[inline(always)]
            pub fn init(tree: &'a Tree) -> Self {
                Self::Init { tree }
            }
        }

        impl<'a> Generator<usize> for DepthGen<'a> {
            type Yield = &'a Tree;
            type Return = usize;

            #[inline(always)]
            fn resume(
                self: std::pin::Pin<&mut Self>,
                child_depth: usize,
            ) -> GeneratorState<Self::Yield, Self::Return> {
                let this = self.get_mut();
                match this {
                    Self::Init { tree } => {
                        let max_child_depth = 0;
                        let mut children = tree.children.iter();
                        if let Some(child) = children.next() {
                            *this = Self::Call {
                                max_child_depth,
                                children,
                            };
                            GeneratorState::Yielded(child)
                        } else {
                            *this = Self::Done;
                            GeneratorState::Complete(max_child_depth + 1)
                        }
                    }
                    Self::Call {
                        max_child_depth,
                        children,
                    } => {
                        *max_child_depth = max(*max_child_depth, child_depth);
                        if let Some(child) = children.next() {
                            GeneratorState::Yielded(child)
                        } else {
                            let depth = *max_child_depth + 1;
                            *this = Self::Done;
                            GeneratorState::Complete(depth)
                        }
                    }
                    Self::Done => panic!("Trying to resume completed DepthGen"),
                }
            }
        }
    }

    pub mod examples {
        use super::*;

        pub fn simple() -> (Tree, usize) {
            let mut v0 = Tree::new(0);
            let v1 = Tree::new(1);
            let mut v2 = Tree::new(2);
            let v3 = Tree::new(3);
            v2.children = vec![v3];
            v0.children = vec![v1, v2];
            (v0, 3)
        }

        pub fn path(n: usize) -> (Tree, usize) {
            let mut tree = Tree::new(n as i64 - 1);
            for k in (0..(n - 1)).rev() {
                let mut parent = Tree::new(k as i64);
                parent.children.push(tree);
                tree = parent;
            }
            (tree, n)
        }

        pub fn binary(n: usize) -> (Tree, usize) {
            if n <= 1 {
                (Tree::new(0), 1)
            } else {
                let mut tree = Tree::new(0);
                tree.children = vec![binary(n - 1).0, binary(n - 1).0];
                (tree, n)
            }
        }
    }
}

pub fn bench_tree_depth(c: &mut Criterion) {
    use tree::*;

    let (simple, simple_depth) = examples::simple();
    assert_eq!(simple.depth_recursive(), simple_depth);
    assert_eq!(simple.depth_stack_safe(), simple_depth);

    #[allow(clippy::type_complexity)]
    let cases: [(&str, fn(usize) -> (Tree, usize), usize); 2] = [
        ("P_{size}", examples::path, 100_000),
        ("B_{size}", examples::binary, 20),
    ];

    let mut group = c.benchmark_group("tree_depth");
    for (label, tree_f, size) in cases {
        let label = label.replace("{size}", &size.to_string());
        let (tree, tree_depth) = tree_f(size);

        assert_eq!(tree.depth_recursive(), tree_depth);
        assert_eq!(
            stack_safe::with_stack_size(1024, move || tree_f(size).0.depth_stack_safe()).unwrap(),
            tree_depth,
        );
        assert_eq!(
            stack_safe::with_stack_size(1024, move || tree_f(size).0.depth_manual()).unwrap(),
            tree_depth,
        );

        group.bench_with_input(BenchmarkId::new("recursive", &label), &tree, |b, tree| {
            b.iter(|| {
                assert_eq!(tree.depth_recursive(), tree_depth);
            })
        });
        group.bench_with_input(BenchmarkId::new("stack_safe", &label), &tree, |b, tree| {
            b.iter(|| {
                assert_eq!(tree.depth_stack_safe(), tree_depth);
            })
        });
        group.bench_with_input(BenchmarkId::new("manual", &label), &tree, |b, tree| {
            b.iter(|| {
                assert_eq!(tree.depth_manual(), tree_depth);
            })
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(2))
        .sample_size(20)
        .configure_from_args();
    targets = bench_tree_depth
}
criterion_main!(benches);
