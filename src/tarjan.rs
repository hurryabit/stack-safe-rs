/*
def stronglyConnectedComponents(graph: Graph) -> List[List[Node]]:
    n = len(graph)
    index = 0
    indices: List[int] = n * [-1]
    lowlinks: List[int] = n * [-1]
    components: List[List[Node]] = []
    stack: List[Node] = []
    on_stack: Set[Node] = set()

    @recurse
    def dfs(v: Node):
        nonlocal index, indices, lowlinks, components, stack
        indices[v] = index
        lowlinks[v] = index
        index += 1
        stack.append(v)
        on_stack.add(v)

        for w in graph[v]:
            if indices[w] < 0:
                yield w
                lowlinks[v] = min(lowlinks[v], lowlinks[w])
            elif w in on_stack:
                lowlinks[v] = min(lowlinks[v], indices[w])

        if lowlinks[v] == indices[v]:
            component: List[Node] = []
            w = -1
            while w != v:
                w = stack.pop()
                on_stack.remove(w)
                component.append(w)
            components.append(component)

    for v in range(n):
        if indices[v] < 0:
            dfs(v)

    return components
*/
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

#[derive(Default)]
struct State {
    index: usize,
    indices: Vec<usize>,
    lowlinks: Vec<usize>,
    components: Vec<Vec<Node>>,
    stack: Vec<Node>,
    on_stack: HashSet<Node>,
}

pub fn tarjan(graph: &Graph) -> Vec<Vec<Node>> {
    use crate::recurse_st;
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
        recurse_st(|v_graph: (Node, &Graph)| {
            let (v, graph) = v_graph;
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

#[cfg(test)]
mod test {
    #![allow(non_upper_case_globals)]
    use super::*;

    const v0: Node = Node::new(0);
    const v1: Node = Node::new(1);
    const v2: Node = Node::new(2);
    const v3: Node = Node::new(3);
    const v4: Node = Node::new(4);

    #[test]
    fn test_simple() {
        let graph = vec![vec![v1], vec![v2, v3], vec![v1, v4], vec![v2], vec![]];
        let expected = vec![vec![v4], vec![v3, v2, v1], vec![v0]];

        assert_eq!(tarjan(&graph), expected);
    }

    #[test]
    // #[ignore = "tarjan is not yet stack safe"]
    fn test_large() {
        #![allow(clippy::needless_range_loop)]
        let n = 10000;
        let mut graph = Vec::with_capacity(n);
        graph.resize_with(n, Vec::new);
        for id in 0..(n - 1) {
            graph[id].push(Node::new(id + 1));
        }
        let mut components = std::thread::Builder::new()
            .stack_size(10240)
            .spawn(move || tarjan(&graph))
            .unwrap()
            .join()
            .unwrap();

        components.reverse();
        for id in 0..n {
            assert_eq!(components[id], vec![Node::new(id)]);
        }
    }
}
