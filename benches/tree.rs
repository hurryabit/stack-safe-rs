#![feature(destructuring_assignment, generators, generator_trait)]
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

mod tree {
    use std::cmp::max;

    #[derive(Clone, Debug)]
    pub struct Tree {
        pub id: usize,
        pub children: Vec<Tree>,
    }

    impl Tree {
        pub fn new(id: usize) -> Self {
            Self {
                id,
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
            stack_safe::recurse(|tree: &Tree| {
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
            let mut tree = Tree::new(n - 1);
            for id in (0..(n - 1)).rev() {
                let mut parent = Tree::new(id);
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
