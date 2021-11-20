#![feature(destructuring_assignment, generators, generator_trait)]
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

mod expr {
    use self::manual::EvalGen;

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

        pub fn eval_stack_safe(&self) -> i64 {
            stack_safe::trampoline(|e: &Self| {
                move |_: i64| match e {
                    Self::Num(n) => *n,
                    Self::Add(e1, e2) => (yield e1.as_ref()) + (yield e2.as_ref()),
                    Self::Mul(e1, e2) => (yield e1.as_ref()) * (yield e2.as_ref()),
                }
            })(self)
        }

        pub fn eval_manual(&self) -> i64 {
            stack_safe::trampoline(EvalGen::init)(self)
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

    pub mod examples {
        use super::Expr;

        pub fn simple() -> (Expr, i64) {
            let expr = Expr::Add(
                Box::new(Expr::Num(1)),
                Box::new(Expr::Mul(Box::new(Expr::Num(2)), Box::new(Expr::Num(3)))),
            );
            (expr, 7)
        }

        pub fn triangular(n: i64) -> (Expr, i64) {
            let mut expr = Expr::Num(0);
            for i in 1..n {
                expr = Expr::Add(Box::new(expr), Box::new(Expr::Num(i)));
            }
            (expr, n * (n - 1) / 2)
        }
    }
}

fn bench_expr_eval(c: &mut Criterion) {
    use expr::*;

    let (simple, simple_eval) = examples::simple();
    assert_eq!(simple.eval_recursive(), simple_eval);

    #[allow(clippy::type_complexity)]
    let cases: [(&str, fn(i64) -> (Expr, i64), i64); 1] =
        [("triangular_{size}", examples::triangular, 100_000)];

    let mut group = c.benchmark_group("expr_eval");
    for (label, expr_f, size) in cases {
        let label = label.replace("{size}", &size.to_string());
        let (expr, expr_eval) = expr_f(size);

        assert_eq!(expr.eval_recursive(), expr_eval);
        stack_safe::with_stack_size(10 * 1024, move || {
            let expr = expr_f(size).0;
            assert_eq!(expr.eval_stack_safe(), expr_eval);
            assert_eq!(expr.eval_manual(), expr_eval);
            assert_eq!(expr.eval_loop(), expr_eval);
            std::mem::forget(expr);
        })
        .unwrap();

        group.bench_with_input(BenchmarkId::new("recursive", &label), &expr, |b, expr| {
            b.iter(|| {
                assert_eq!(expr.eval_recursive(), expr_eval);
            })
        });
        group.bench_with_input(BenchmarkId::new("stack_safe", &label), &expr, |b, expr| {
            b.iter(|| {
                assert_eq!(expr.eval_stack_safe(), expr_eval);
            })
        });
        group.bench_with_input(BenchmarkId::new("manual", &label), &expr, |b, expr| {
            b.iter(|| {
                assert_eq!(expr.eval_manual(), expr_eval);
            })
        });
        group.bench_with_input(BenchmarkId::new("loop", &label), &expr, |b, expr| {
            b.iter(|| {
                assert_eq!(expr.eval_loop(), expr_eval);
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
    targets = bench_expr_eval
}
criterion_main!(benches);
