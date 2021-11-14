#![feature(destructuring_assignment, generators, generator_trait)]
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

mod tarjan {
    use std::collections::HashSet;

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub struct Node {
        id: usize,
    }

    impl Node {
        pub const fn new(id: usize) -> Self {
            Self { id }
        }
    }

    pub type Graph = Vec<Vec<Node>>;

    pub type SCCs = Vec<Vec<Node>>;

    #[derive(Default)]
    struct State {
        index: usize,
        indices: Vec<usize>,
        lowlinks: Vec<usize>,
        components: SCCs,
        stack: Vec<Node>,
        on_stack: HashSet<Node>,
    }

    pub fn recursive(graph: &Graph) -> SCCs {
        use std::cmp::min;

        let n = graph.len();
        let mut s = State {
            index: 0,
            indices: Vec::with_capacity(n),
            lowlinks: Vec::with_capacity(n),
            components: Vec::new(),
            stack: Vec::new(),
            on_stack: HashSet::new(),
        };
        s.indices.resize(n, usize::MAX);
        s.lowlinks.resize(n, usize::MAX);

        fn dfs(v: Node, graph: &Graph, s: &mut State) {
            s.indices[v.id] = s.index;
            s.lowlinks[v.id] = s.index;
            s.index += 1;
            s.stack.push(v);
            s.on_stack.insert(v);

            for &w in &graph[v.id] {
                if s.indices[w.id] == usize::MAX {
                    dfs(w, graph, s);
                    s.lowlinks[v.id] = min(s.lowlinks[v.id], s.lowlinks[w.id]);
                } else if s.on_stack.contains(&w) {
                    s.lowlinks[v.id] = min(s.lowlinks[v.id], s.indices[w.id]);
                }
            }

            if s.lowlinks[v.id] == s.indices[v.id] {
                let mut component = Vec::new();
                let mut w = Node { id: usize::MAX };
                while w != v {
                    w = s.stack.pop().unwrap();
                    s.on_stack.remove(&w);
                    component.push(w)
                }
                s.components.push(component);
            }
        }

        for id in 0..n {
            let v = Node { id };
            if s.indices[v.id] == usize::MAX {
                dfs(v, graph, &mut s);
            }
        }

        s.components
    }

    pub fn stack_safe(graph: &Graph) -> SCCs {
        use stack_safe::recurse_st;
        use std::cmp::min;

        let n = graph.len();
        let mut s = State {
            index: 0,
            indices: Vec::with_capacity(n),
            lowlinks: Vec::with_capacity(n),
            components: Vec::new(),
            stack: Vec::new(),
            on_stack: HashSet::new(),
        };
        s.indices.resize(n, usize::MAX);
        s.lowlinks.resize(n, usize::MAX);

        fn dfs(v: Node, graph: &Graph, s: &mut State) {
            recurse_st(|(v, graph): (Node, &Graph)| {
                move |(_, mut s): ((), State)| {
                    s.indices[v.id] = s.index;
                    s.lowlinks[v.id] = s.index;
                    s.index += 1;
                    s.stack.push(v);
                    s.on_stack.insert(v);

                    for &w in &graph[v.id] {
                        if s.indices[w.id] == usize::MAX {
                            ((), s) = yield ((w, graph), s);
                            s.lowlinks[v.id] = min(s.lowlinks[v.id], s.lowlinks[w.id]);
                        } else if s.on_stack.contains(&w) {
                            s.lowlinks[v.id] = min(s.lowlinks[v.id], s.indices[w.id]);
                        }
                    }

                    if s.lowlinks[v.id] == s.indices[v.id] {
                        let mut component = Vec::new();
                        let mut w = Node { id: usize::MAX };
                        while w != v {
                            w = s.stack.pop().unwrap();
                            s.on_stack.remove(&w);
                            component.push(w)
                        }
                        s.components.push(component);
                    }
                    ((), s)
                }
            })((v, graph), s)
        }

        for id in 0..n {
            let v = Node { id };
            if s.indices[v.id] == usize::MAX {
                dfs(v, graph, &mut s);
            }
        }

        s.components
    }

    pub mod examples {
        #![allow(non_upper_case_globals)]
        use super::*;

        const v0: Node = Node::new(0);
        const v1: Node = Node::new(1);
        const v2: Node = Node::new(2);
        const v3: Node = Node::new(3);
        const v4: Node = Node::new(4);

        pub fn simple() -> Graph {
            vec![vec![v1], vec![v2, v3], vec![v1, v4], vec![v2], vec![]]
        }

        pub fn simple_sccs() -> SCCs {
            vec![vec![v4], vec![v3, v2, v1], vec![v0]]
        }

        pub fn path(n: usize) -> Graph {
            let mut graph: Vec<_> = (1..n).map(|id| vec![Node::new(id)]).collect();
            graph.push(vec![]);
            graph
        }

        pub fn path_sccs(n: usize) -> SCCs {
            (0..n).rev().map(|id| vec![Node::new(id)]).collect()
        }

        pub fn path_rev(n: usize) -> Graph {
            let mut graph = vec![vec![]];
            graph.extend((0..(n-1)).map(|id| vec![Node::new(id)]));
            graph
        }

        pub fn path_rev_sccs(n: usize) -> SCCs {
            (0..n).map(|id| vec![Node::new(id)]).collect()
        }

        pub fn complete(n: usize) -> Graph {
            let outgoing: Vec<_> = (0..n).map(Node::new).collect();
            (0..n).map(|_| outgoing.clone()).collect()
        }

        pub fn complete_sccs(n: usize) -> SCCs {
            vec![(0..n).rev().map(Node::new).collect()]
        }
    }
}

pub fn bench_tarjan(c: &mut Criterion) {
    use tarjan::*;

    assert_eq!(recursive(&examples::simple()), examples::simple_sccs());
    assert_eq!(stack_safe(&examples::simple()), examples::simple_sccs());

    #[allow(clippy::type_complexity)]
    let cases: [(&str, fn(usize) -> Graph, fn(usize) -> SCCs, usize); 3] = [
        ("P_{size}", examples::path, examples::path_sccs, 10_000),
        ("P_{size}_rev", examples::path_rev, examples::path_rev_sccs, 10_000),
        ("K_{size}", examples::complete, examples::complete_sccs, 500),
    ];

    let mut group = c.benchmark_group("tarjan");
    for (label, graph, sccs, size) in cases {
        let label = label.replace("{size}", &size.to_string());
        let graph = graph(size);
        let sccs = sccs(size);

        assert_eq!(recursive(&graph), sccs);
        let graph_clone = graph.clone();
        assert_eq!(
            stack_safe::with_stack_size(10 * 1024, move || stack_safe(&graph_clone)).unwrap(),
            sccs,
        );

        group.bench_with_input(BenchmarkId::new("recursive", &label), &graph, |b, graph| {
            b.iter(|| {
                recursive(graph);
            })
        });
        group.bench_with_input(
            BenchmarkId::new("stack_safe", &label),
            &graph,
            |b, graph| {
                b.iter(|| {
                    stack_safe(graph);
                })
            },
        );
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
    targets = bench_tarjan
}
criterion_main!(benches);
