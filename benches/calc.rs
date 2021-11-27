#![feature(generators, generator_trait)]
#![allow(clippy::unnecessary_cast)]
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::ops::{Generator, GeneratorState};
use std::pin::Pin;
use std::time::Duration;

pub type Num = f64;

pub enum Expr {
    Num(Num),
    Add(Box<Self>, Box<Self>),
    Mul(Box<Self>, Box<Self>),
}

impl Drop for Expr {
    fn drop(&mut self) {
        std::mem::forget(std::mem::take(self));
    }
}

impl Default for Expr {
    fn default() -> Self {
        Self::Num(0 as Num)
    }
}

impl Expr {
    pub fn eval_recursive(&self) -> Num {
        match self {
            Self::Num(num) => *num,
            Self::Add(expr1, expr2) => expr1.eval_recursive() + expr2.eval_recursive(),
            Self::Mul(expr1, expr2) => expr1.eval_recursive() * expr2.eval_recursive(),
        }
    }
}

impl Expr {
    pub fn eval_trampolined<'a>(&'a self) -> Num {
        trampoline(|e: &'a Self| {
            move |_| match e {
                Self::Num(n) => *n,
                Self::Add(e1, e2) => (yield e1.as_ref()) + (yield e2.as_ref()),
                Self::Mul(e1, e2) => (yield e1.as_ref()) * (yield e2.as_ref()),
            }
        })(self)
    }

    pub fn eval_trampolined_opt(&self) -> Num {
        pub enum Gen<'a> {
            Init { expr: &'a Expr },
            Add1 { rhs: &'a Expr },
            Add2 { lhs: Num },
            Mul1 { rhs: &'a Expr },
            Mul2 { lhs: Num },
            Done,
        }

        impl<'a> Gen<'a> {
            pub fn init(expr: &'a Expr) -> Self {
                Self::Init { expr }
            }
        }

        impl<'a> Generator<Num> for Gen<'a> {
            type Yield = &'a Expr;
            type Return = Num;

            fn resume(
                self: std::pin::Pin<&mut Self>,
                val: Num,
            ) -> std::ops::GeneratorState<Self::Yield, Self::Return> {
                let this = self.get_mut();
                match this {
                    Self::Init { expr } => match expr {
                        Expr::Num(num) => {
                            *this = Self::Done;
                            GeneratorState::Complete(*num)
                        }
                        Expr::Add(lhs, rhs) => {
                            *this = Self::Add1 { rhs };
                            GeneratorState::Yielded(lhs)
                        }
                        Expr::Mul(lhs, rhs) => {
                            *this = Self::Mul1 { rhs };
                            GeneratorState::Yielded(lhs)
                        }
                    },
                    Self::Add1 { rhs } => {
                        let rhs = *rhs;
                        *this = Self::Add2 { lhs: val };
                        GeneratorState::Yielded(rhs)
                    }
                    Self::Add2 { lhs } => {
                        let lhs = *lhs;
                        *this = Self::Done;
                        GeneratorState::Complete(lhs + val)
                    }
                    Self::Mul1 { rhs } => {
                        let rhs = *rhs;
                        *this = Self::Mul2 { lhs: val };
                        GeneratorState::Yielded(rhs)
                    }
                    Self::Mul2 { lhs } => {
                        let lhs = *lhs;
                        *this = Self::Done;
                        GeneratorState::Complete(lhs * val)
                    }
                    Self::Done => panic!("resuming finished generator"),
                }
            }
        }

        trampoline(Gen::init)(self)
    }
}

impl Expr {
    pub fn eval_iterative_cps(&self) -> Num {
        enum Cont<'a> {
            AddLhs { rhs: &'a Expr },
            AddRhs { lhs: Num },
            MulLhs { rhs: &'a Expr },
            MulRhs { lhs: Num },
        }

        let mut cont_chain = Vec::new();
        let mut expr = self;
        loop {
            match expr {
                Self::Num(num) => {
                    let mut val = *num;
                    loop {
                        if let Some(cont) = cont_chain.pop() {
                            match cont {
                                Cont::AddLhs { rhs } => {
                                    cont_chain.push(Cont::AddRhs { lhs: val });
                                    expr = rhs;
                                    break;
                                }
                                Cont::AddRhs { lhs } => {
                                    val += lhs;
                                }
                                Cont::MulLhs { rhs } => {
                                    cont_chain.push(Cont::MulRhs { lhs: val });
                                    expr = rhs;
                                    break;
                                }
                                Cont::MulRhs { lhs } => {
                                    val *= lhs;
                                }
                            }
                        } else {
                            return val;
                        }
                    }
                }
                Self::Add(lhs, rhs) => {
                    cont_chain.push(Cont::AddLhs { rhs });
                    expr = lhs;
                }
                Self::Mul(lhs, rhs) => {
                    cont_chain.push(Cont::MulLhs { rhs });
                    expr = lhs;
                }
            }
        }
    }
}

impl Expr {
    pub fn eval_iterative_rpn(&self) -> Num {
        enum Item<'a> {
            Operand(&'a Expr),
            Add,
            Mul,
        }

        let mut current_expr = self;
        let mut item_stack = Vec::new();
        let mut value_stack = Vec::new();
        let mut current_val = 0 as Num;

        loop {
            match current_expr {
                Expr::Num(val) => {
                    value_stack.push(current_val);
                    current_val = *val;
                    loop {
                        if let Some(item) = item_stack.pop() {
                            match item {
                                Item::Operand(expr) => {
                                    current_expr = expr;
                                    break;
                                }
                                Item::Add => {
                                    current_val += value_stack.pop().unwrap();
                                }
                                Item::Mul => {
                                    current_val *= value_stack.pop().unwrap();
                                }
                            }
                        } else {
                            return current_val;
                        }
                    }
                }
                Expr::Add(lhs, rhs) => {
                    item_stack.push(Item::Add);
                    item_stack.push(Item::Operand(rhs));
                    current_expr = lhs;
                }
                Expr::Mul(lhs, rhs) => {
                    item_stack.push(Item::Mul);
                    item_stack.push(Item::Operand(rhs));
                    current_expr = lhs;
                }
            }
        }
    }
}

mod examples {
    use std::fmt::Display;

    use super::*;
    use rand::random;

    pub struct Case {
        pub expr: Expr,
        pub eval: Num,
        pub size: usize,
    }

    #[derive(Clone, Copy)]
    pub enum Ops {
        Add,
        Mul,
        Rnd,
    }

    impl Display for Ops {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Add => write!(f, "add"),
                Self::Mul => write!(f, "mul"),
                Self::Rnd => write!(f, "rnd"),
            }
        }
    }

    // impl Ops {
    //     fn random(self) -> (&'static dyn Fn (Box<Expr>, Box<Expr>) -> Expr, fn (Num, Num) -> Num) {
    //         use std::ops::Add;

    //         let add = (&Expr::Add, Num::add);
    //         match self {
    //             Ops::Add => add,
    //             Ops::Mul => todo!(),
    //             Ops::Both => todo!(),
    //         }
    //     }
    // }

    pub fn simple() -> (Expr, Num) {
        let expr = {
            Expr::Add(
                Box::new(Expr::Num(1 as Num)),
                Box::new(Expr::Mul(
                    Box::new(Expr::Num(2 as Num)),
                    Box::new(Expr::Num(3 as Num)),
                )),
            )
        };
        (expr, 7 as Num)
    }

    fn random_num() -> Case {
        let num = random();
        Case {
            expr: Expr::Num(num),
            eval: num,
            size: 1,
        }
    }

    fn random_bin(ops: Ops, lhs: Case, rhs: Case) -> Case {
        let lhs_expr = Box::new(lhs.expr);
        let rhs_expr = Box::new(rhs.expr);
        let add = match ops {
            Ops::Add => true,
            Ops::Mul => false,
            Ops::Rnd => random(),
        };
        let (expr, eval) = if add {
            (Expr::Add(lhs_expr, rhs_expr), lhs.eval + rhs.eval)
        } else {
            (Expr::Mul(lhs_expr, rhs_expr), lhs.eval * rhs.eval)
        };
        let size = 1 + lhs.size + rhs.size;
        Case { expr, eval, size }
    }

    fn tree_with(ops: Ops, n: usize, leaf: &dyn Fn() -> Case) -> Case {
        if n == 0 {
            leaf()
        } else {
            random_bin(
                ops,
                tree_with(ops, n - 1, leaf),
                tree_with(ops, n - 1, leaf),
            )
        }
    }

    fn branch_with(ops: Ops, n: usize, leaf: &dyn Fn() -> Case) -> Case {
        let mut expr = leaf();
        for _ in 0..n {
            expr = if random::<bool>() {
                random_bin(ops, expr, leaf())
            } else {
                random_bin(ops, leaf(), expr)
            }
        }
        expr
    }

    pub fn one_branch(ops: Ops) -> Case {
        branch_with(ops, 512 * 1024 - 1, &random_num)
    }

    pub fn many_trees(ops: Ops) -> Case {
        branch_with(ops, 1023, &|| tree_with(ops, 9, &random_num))
    }

    pub fn one_tree(ops: Ops) -> Case {
        tree_with(ops, 19, &random_num)
    }

    pub fn many_branches(ops: Ops) -> Case {
        tree_with(ops, 10, &|| branch_with(ops, 511, &random_num))
    }
}

fn bench_expr_eval(c: &mut Criterion) {
    #![allow(clippy::type_complexity)]
    use examples::*;

    let implementations: &[(&str, fn(&Expr) -> Num)] = &[
        ("recursive", Expr::eval_recursive),
        ("trampolined", Expr::eval_trampolined),
        ("trampolined_opt", Expr::eval_trampolined_opt),
        ("iterative_cps", Expr::eval_iterative_cps),
        // ("iterative_rpn", Expr::eval_iterative_rpn),
    ];

    let (simple, simple_eval) = examples::simple();
    for (_, impl_func) in implementations {
        assert_eq!(impl_func(&simple), simple_eval);
    }

    let cases: &[(&str, fn(examples::Ops) -> examples::Case)] = &[
        ("one_tree", examples::one_tree),
        ("one_branch", examples::one_branch),
        ("many_trees", examples::many_trees),
        ("many_branches", examples::many_branches),
    ];

    let group_name = format!("expr_{}", std::any::type_name::<Num>());
    let mut group = c.benchmark_group(&group_name);
    for (case_name, case_func) in cases {
        for ops in [Ops::Add, Ops::Rnd] {
            let case_name = format!("{}_{}", case_name, ops);
            let case1 = case_func(ops);
            let case2 = case_func(ops);
            println!("{}/{} has size {}", group_name, case_name, case1.size);

            assert_eq!(case1.expr.eval_recursive(), case1.eval);
            stack_safe::with_stack_size(10 * 1024, || {
                for (impl_name, impl_func) in &implementations[1..] {
                    assert_eq!(
                        impl_func(&case1.expr),
                        case1.eval,
                        "testing implementation {} on case {}",
                        impl_name,
                        case_name,
                    );
                }
            })
            .unwrap();

            let cases = (case1, case2);
            for (impl_name, impl_func) in implementations {
                group.bench_with_input(
                    BenchmarkId::new(*impl_name, &case_name),
                    &cases,
                    |b, (case1, case2)| {
                        b.iter(|| {
                            assert_eq!(impl_func(&case1.expr), case1.eval);
                            assert_eq!(impl_func(&case2.expr), case2.eval);
                        })
                    },
                );
            }
        }
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(60))
        .warm_up_time(Duration::from_secs(5))
        .sample_size(50)
        .configure_from_args();
    targets = bench_expr_eval
}
criterion_main!(benches);

pub fn trampoline<Arg, Res, Gen>(f: impl Fn(Arg) -> Gen) -> impl Fn(Arg) -> Res
where
    Res: Default,
    Gen: Generator<Res, Yield = Arg, Return = Res> + Unpin,
{
    move |arg: Arg| {
        let mut stack = Vec::new();
        let mut current = f(arg);
        let mut res = Res::default();

        loop {
            match Pin::new(&mut current).resume(res) {
                GeneratorState::Yielded(arg) => {
                    stack.push(current);
                    current = f(arg);
                    res = Res::default();
                }
                GeneratorState::Complete(real_res) => match stack.pop() {
                    None => return real_res,
                    Some(top) => {
                        current = top;
                        res = real_res;
                    }
                },
            }
        }
    }
}
