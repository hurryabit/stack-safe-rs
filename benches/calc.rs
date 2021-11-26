#![feature(generators, generator_trait)]
#![allow(clippy::unnecessary_cast)]
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

mod expr {
    pub type Num = i64;

    pub enum Expr {
        Num(Num),
        Add(Box<Expr>, Box<Expr>),
        Mul(Box<Expr>, Box<Expr>),
    }

    impl Default for Expr {
        fn default() -> Self {
            Self::Num(0 as Num)
        }
    }

    impl Expr {
        pub fn eval_recursive(&self) -> Num {
            match self {
                Self::Num(n) => *n,
                Self::Add(e1, e2) => e1.eval_recursive() + e2.eval_recursive(),
                Self::Mul(e1, e2) => e1.eval_recursive() * e2.eval_recursive(),
            }
        }

        pub fn eval_stack_safe<'a>(&'a self) -> Num {
            let gen = |e: &'a Self| {
                move |_| match e {
                    Self::Num(n) => *n,
                    Self::Add(e1, e2) => (yield e1.as_ref()) + (yield e2.as_ref()),
                    Self::Mul(e1, e2) => (yield e1.as_ref()) * (yield e2.as_ref()),
                }
            };
            static_assertions::assert_eq_size_val!(gen(self), [0u8; 40]);
            stack_safe::trampoline(gen)(self)
        }

        pub fn eval_optimal_gen(&self) -> Num {
            stack_safe::trampoline(optimal_gen::EvalGen::init)(self)
        }

        pub fn eval_loop(&self) -> Num {
            enum Ctrl<'a> {
                Expr(&'a Expr),
                Value(Num),
            }

            enum Kont<'a> {
                Add1(&'a Expr),
                Add2(Num),
                Mul1(&'a Expr),
                Mul2(Num),
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
    mod optimal_gen {
        use super::*;
        use std::ops::{Generator, GeneratorState};

        pub enum EvalGen<'a> {
            Init(&'a Expr),
            Add1(&'a Expr),
            Add2(Num),
            Mul1(&'a Expr),
            Mul2(Num),
            Done,
        }

        impl<'a> EvalGen<'a> {
            pub fn init(expr: &'a Expr) -> Self {
                Self::Init(expr)
            }
        }

        static_assertions::assert_eq_size!(EvalGen, [u8; 16]);

        impl<'a> Generator<Num> for EvalGen<'a> {
            type Yield = &'a Expr;
            type Return = Num;

            fn resume(
                self: std::pin::Pin<&mut Self>,
                value: Num,
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

    pub mod examples {
        use super::*;

        pub fn simple() -> (Expr, Num) {
            let expr = Expr::Add(
                Box::new(Expr::Num(1 as Num)),
                Box::new(Expr::Mul(
                    Box::new(Expr::Num(2 as Num)),
                    Box::new(Expr::Num(3 as Num)),
                )),
            );
            (expr, 7 as Num)
        }

        pub fn triangular(n: usize) -> (Expr, Num) {
            let mut expr = Expr::Num(0 as Num);
            for i in 1..=n {
                expr = Expr::Add(Box::new(expr), Box::new(Expr::Num(i as Num)));
            }
            (expr, (n as Num) * (n as Num + 1 as Num) / 2 as Num)
        }

        pub fn power(n: usize) -> (Expr, Num) {
            let mut expr = Expr::Num(1 as Num);
            let mut eval = 1 as Num;
            for _ in 0..n {
                expr = Expr::Mul(Box::new(expr), Box::new(Expr::Num(2 as Num)));
                eval *= 2 as Num;
            }
            (expr, eval)
        }

        pub fn complete_tree(n: usize) -> (Expr, Num) {
            complete_tree_with(n, || {
                (
                    Expr::Mul(Box::new(Expr::Num(1 as Num)), Box::new(Expr::Num(1 as Num))),
                    1 as Num,
                )
            })
        }

        pub fn complete_tree_with(n: usize, leaf: impl Fn() -> (Expr, Num)) -> (Expr, Num) {
            if n == 0 {
                leaf()
            } else {
                let (lhs, lhs_eval) = complete_tree(n - 1);
                let (rhs, rhs_eval) = complete_tree(n - 1);
                (Expr::Add(Box::new(lhs), Box::new(rhs)), lhs_eval + rhs_eval)
            }
        }

        pub fn mixed(n: usize) -> (Expr, Num) {
            let m = 10 * 2usize.pow(n as u32);
            complete_tree_with(n, || triangular(m))
        }
    }
}

fn bench_expr_eval(c: &mut Criterion) {
    #![allow(clippy::type_complexity)]
    use expr::*;

    let implementations: [(&str, fn(&Expr) -> Num); 4] = [
        ("recursive", Expr::eval_recursive),
        ("loop", Expr::eval_loop),
        ("stack_safe", Expr::eval_stack_safe),
        ("optimal_gen", Expr::eval_optimal_gen),
    ];

    let (simple, simple_eval) = examples::simple();
    for (_, impl_func) in implementations {
        assert_eq!(impl_func(&simple), simple_eval);
    }

    let cases: [(&str, fn(usize) -> (Expr, Num), usize); 4] = [
        ("triangular_{size}", examples::triangular, 100_000),
        ("power_{size}", examples::power, 100_000),
        ("complete_tree_{size}", examples::complete_tree, 18),
        ("mixed_{size}", examples::mixed, 17),
    ];

    let mut group = c.benchmark_group(format!("eval_{}", std::any::type_name::<Num>()));
    for (case_name, case_func, case_size) in cases {
        let case_name = &case_name.replace("{size}", &case_size.to_string());
        let (expr1, expr1_eval) = case_func(case_size);
        let (expr2, expr2_eval) = case_func((2 * (case_size + 1) - 2) / 2);

        assert_eq!(expr1.eval_recursive(), expr1_eval);
        stack_safe::with_stack_size(10 * 1024, move || {
            let expr = case_func(case_size).0;
            assert_eq!(expr.eval_stack_safe(), expr1_eval);
            assert_eq!(expr.eval_optimal_gen(), expr1_eval);
            assert_eq!(expr.eval_loop(), expr1_eval);
            std::mem::forget(expr);
        })
        .unwrap();

        let exprs = (expr1, expr2);
        for (impl_name, impl_func) in implementations {
            group.bench_with_input(
                BenchmarkId::new(case_name, impl_name),
                &exprs,
                |b, (expr1, expr2)| {
                    b.iter(|| {
                        assert_eq!(impl_func(expr1), expr1_eval);
                        assert_eq!(impl_func(expr2), expr2_eval);
                    })
                },
            );
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
