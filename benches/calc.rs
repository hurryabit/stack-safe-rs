#![feature(destructuring_assignment, generators, generator_trait)]
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

mod expr {
    pub enum Expr {
        Num(i64),
        Add(Box<Expr>, Box<Expr>),
        Mul(Box<Expr>, Box<Expr>),
    }

    impl Default for Expr {
        fn default() -> Self {
            Self::Num(0)
        }
    }

    impl Expr {
        pub fn eval_recursive(&self) -> i64 {
            match self {
                Self::Num(n) => *n,
                Self::Add(e1, e2) => e1.eval_recursive() + e2.eval_recursive(),
                Self::Mul(e1, e2) => e1.eval_recursive() * e2.eval_recursive(),
            }
        }

        pub fn eval_stack_safe<'a>(&'a self) -> i64 {
            let gen = |e: &'a Self| {
                move |_: i64| match e {
                    Self::Num(n) => *n,
                    Self::Add(e1, e2) => (yield e1.as_ref()) + (yield e2.as_ref()),
                    Self::Mul(e1, e2) => (yield e1.as_ref()) * (yield e2.as_ref()),
                }
            };
            static_assertions::assert_eq_size_val!(gen(self), [0u8; 40]);
            stack_safe::trampoline(gen)(self)
        }

        pub fn eval_manual(&self) -> i64 {
            stack_safe::trampoline(manual::EvalGen::init)(self)
        }

        pub fn eval_like_generated(&self) -> i64 {
            stack_safe::trampoline(like_generated::EvalGen::init)(self)
        }

        pub fn eval_loop(&self) -> i64 {
            enum Ctrl<'a> {
                Expr(&'a Expr),
                Value(i64),
            }

            enum Kont<'a> {
                Add1(&'a Expr),
                Add2(i64),
                Mul1(&'a Expr),
                Mul2(i64),
            }

            let mut stack = Vec::new();
            let mut ctrl = Ctrl::Expr(self);
            loop {
                match ctrl {
                    Ctrl::Expr(e) => match e {
                        Self::Num(n) => {
                            ctrl = Ctrl::Value(*n);
                        }
                        Self::Add(e1, e2) => {
                            stack.push(Kont::Add1(e2));
                            ctrl = Ctrl::Expr(e1);
                        }
                        Self::Mul(e1, e2) => {
                            stack.push(Kont::Mul1(e2));
                            ctrl = Ctrl::Expr(e1);
                        }
                    },
                    Ctrl::Value(v) => {
                        if let Some(kont) = stack.pop() {
                            match kont {
                                Kont::Add1(e2) => {
                                    stack.push(Kont::Add2(v));
                                    ctrl = Ctrl::Expr(e2);
                                }
                                Kont::Add2(v1) => {
                                    ctrl = Ctrl::Value(v1 + v);
                                }
                                Kont::Mul1(e2) => {
                                    stack.push(Kont::Mul2(v));
                                    ctrl = Ctrl::Expr(e2);
                                }
                                Kont::Mul2(v1) => {
                                    ctrl = Ctrl::Value(v1 * v);
                                }
                            }
                        } else {
                            break v;
                        }
                    }
                }
            }
        }
    }

    // This module contains a hand-written generator for evaluation.
    mod manual {
        use super::*;
        use std::ops::{Generator, GeneratorState};

        pub enum EvalGen<'a> {
            Init(&'a Expr),
            Add1(&'a Expr),
            Add2(i64),
            Mul1(&'a Expr),
            Mul2(i64),
            Done,
        }

        impl<'a> EvalGen<'a> {
            pub fn init(expr: &'a Expr) -> Self {
                Self::Init(expr)
            }
        }

        static_assertions::assert_eq_size!(EvalGen, [u8; 16]);

        impl<'a> Generator<i64> for EvalGen<'a> {
            type Yield = &'a Expr;
            type Return = i64;

            fn resume(
                self: std::pin::Pin<&mut Self>,
                value: i64,
            ) -> std::ops::GeneratorState<Self::Yield, Self::Return> {
                let this = self.get_mut();
                match this {
                    Self::Init(e) => match e {
                        Expr::Num(n) => {
                            *this = Self::Done;
                            GeneratorState::Complete(*n)
                        }
                        Expr::Add(e1, e2) => {
                            *this = Self::Add1(e2);
                            GeneratorState::Yielded(e1)
                        }
                        Expr::Mul(e1, e2) => {
                            *this = Self::Mul1(e2);
                            GeneratorState::Yielded(e1)
                        }
                    },
                    Self::Add1(e2) => {
                        let e2 = *e2;
                        *this = Self::Add2(value);
                        GeneratorState::Yielded(e2)
                    }
                    Self::Add2(v1) => {
                        let v1 = *v1;
                        *this = Self::Done;
                        GeneratorState::Complete(v1 + value)
                    }
                    Self::Mul1(e2) => {
                        let e2 = *e2;
                        *this = Self::Mul2(value);
                        GeneratorState::Yielded(e2)
                    }
                    Self::Mul2(v1) => {
                        let v1 = *v1;
                        *this = Self::Done;
                        GeneratorState::Complete(v1 * value)
                    }
                    Self::Done => panic!("Trying to resume completed EvalGen generator."),
                }
            }
        }
    }

    // This module contains a generator for evaluation that resembles the one
    // produced by the compiler from the `yield` version. In particular, it
    // takes as much space as the produced one.
    mod like_generated {
        use super::*;
        use std::ops::{Generator, GeneratorState};

        pub enum EvalGenState {
            Init,
            Add1,
            Add2,
            Mul1,
            Mul2,
            Done,
        }

        pub struct EvalGen<'a> {
            state: EvalGenState,
            e0: &'a Expr,
            e2: &'a Expr,
            v1_add: i64,
            v1_mul: i64,
        }

        static_assertions::assert_eq_size!(EvalGen, [u8; 40]);

        impl<'a> EvalGen<'a> {
            pub fn init(expr: &'a Expr) -> Self {
                Self {
                    state: EvalGenState::Init,
                    e0: expr,
                    e2: expr,
                    v1_add: 0,
                    v1_mul: 0,
                }
            }
        }

        impl<'a> Generator<i64> for EvalGen<'a> {
            type Yield = &'a Expr;
            type Return = i64;

            fn resume(
                self: std::pin::Pin<&mut Self>,
                value: i64,
            ) -> std::ops::GeneratorState<Self::Yield, Self::Return> {
                let this = self.get_mut();
                match this.state {
                    EvalGenState::Init => match this.e0 {
                        Expr::Num(n) => {
                            this.state = EvalGenState::Done;
                            GeneratorState::Complete(*n)
                        }
                        Expr::Add(e1, e2) => {
                            this.e2 = e2;
                            this.state = EvalGenState::Add1;
                            GeneratorState::Yielded(e1)
                        }
                        Expr::Mul(e1, e2) => {
                            this.e2 = e2;
                            this.state = EvalGenState::Mul1;
                            GeneratorState::Yielded(e1)
                        }
                    },
                    EvalGenState::Add1 => {
                        this.v1_add = value;
                        this.state = EvalGenState::Add2;
                        GeneratorState::Yielded(this.e2)
                    }
                    EvalGenState::Add2 => {
                        this.state = EvalGenState::Done;
                        GeneratorState::Complete(this.v1_add + value)
                    }
                    EvalGenState::Mul1 => {
                        this.v1_mul = value;
                        this.state = EvalGenState::Mul2;
                        GeneratorState::Yielded(this.e2)
                    }
                    EvalGenState::Mul2 => {
                        this.state = EvalGenState::Done;
                        GeneratorState::Complete(this.v1_mul * value)
                    }
                    EvalGenState::Done => panic!("Trying to resume completed EvalGen generator."),
                }
            }
        }
    }

    pub mod examples {
        use super::Expr;

        pub fn simple() -> (Expr, i64) {
            let expr = Expr::Add(
                Box::new(Expr::Num(1)),
                Box::new(Expr::Mul(Box::new(Expr::Num(2)), Box::new(Expr::Num(3)))),
            );
            (expr, 7)
        }

        pub fn triangular(n: usize) -> (Expr, i64) {
            let n = n as i64;
            let mut expr = Expr::Num(0);
            for i in 1..n {
                expr = Expr::Add(Box::new(expr), Box::new(Expr::Num(i)));
            }
            (expr, n * (n - 1) / 2)
        }

        pub fn complete_tree(n: usize) -> (Expr, i64) {
            complete_tree_with(n, || (Expr::Num(1), 1))
        }

        pub fn complete_tree_with(n: usize, leaf: impl Fn() -> (Expr, i64)) -> (Expr, i64) {
            if n == 0 {
                leaf()
            } else {
                let (lhs, lhs_eval) = complete_tree(n - 1);
                let (rhs, rhs_eval) = complete_tree(n - 1);
                (Expr::Add(Box::new(lhs), Box::new(rhs)), lhs_eval + rhs_eval)
            }
        }

        pub fn mixed(n: usize) -> (Expr, i64) {
            let m = 10 * 2usize.pow(n as u32);
            complete_tree_with(n, || triangular(m))
        }
    }
}

fn bench_expr_eval(c: &mut Criterion) {
    #![allow(clippy::type_complexity)]
    use expr::*;

    let implementations: [(&str, fn(&Expr) -> i64); 5] = [
        ("recursive", Expr::eval_recursive),
        ("stack_safe", Expr::eval_stack_safe),
        ("manual", Expr::eval_manual),
        ("like_generated", Expr::eval_like_generated),
        ("loop", Expr::eval_loop),
    ];

    let (simple, simple_eval) = examples::simple();
    for (_, impl_func) in implementations {
        assert_eq!(impl_func(&simple), simple_eval);
    }

    let cases: [(&str, fn(usize) -> (Expr, i64), usize); 3] = [
        ("triangular_{size}", examples::triangular, 100_000),
        ("complete_tree_{size}", examples::complete_tree, 20),
        ("mixed_{size}", examples::mixed, 17),
    ];


    let mut group = c.benchmark_group("expr_eval");
    for (case_name, case_func, case_size) in cases {
        let case_name = &case_name.replace("{size}", &case_size.to_string());
        let (expr1, expr1_eval) = case_func(case_size);
        let (expr2, expr2_eval) = case_func((2 * (case_size + 1) - 2) / 2);

        assert_eq!(expr1.eval_recursive(), expr1_eval);
        stack_safe::with_stack_size(10 * 1024, move || {
            let expr = case_func(case_size).0;
            assert_eq!(expr.eval_stack_safe(), expr1_eval);
            assert_eq!(expr.eval_manual(), expr1_eval);
            assert_eq!(expr.eval_like_generated(), expr1_eval);
            assert_eq!(expr.eval_loop(), expr1_eval);
            std::mem::forget(expr);
        })
        .unwrap();

        let exprs = (expr1, expr2);
        for (impl_name, impl_func) in implementations {
            group.bench_with_input(BenchmarkId::new(impl_name, case_name), &exprs, |b, (expr1, expr2)| {
                b.iter(|| {
                    assert_eq!(impl_func(expr1), expr1_eval);
                    assert_eq!(impl_func(expr2), expr2_eval);
                })
            });
        }
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
    targets = bench_expr_eval
}
criterion_main!(benches);
