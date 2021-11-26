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
    use super::*;

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

    pub fn complete(n: usize) -> (Expr, Num) {
        assert!(n >= 1);
        tree_with(n - 1, &|| {
            (
                Expr::Mul(Box::new(Expr::Num(1 as Num)), Box::new(Expr::Num(1 as Num))),
                1 as Num,
            )
        })
    }

    pub fn tree_with(n: usize, leaf: &dyn Fn() -> (Expr, Num)) -> (Expr, Num) {
        if n == 0 {
            leaf()
        } else {
            let (lhs, lhs_eval) = tree_with(n - 1, leaf);
            let (rhs, rhs_eval) = tree_with(n - 1, leaf);
            (Expr::Add(Box::new(lhs), Box::new(rhs)), lhs_eval + rhs_eval)
        }
    }

    pub fn mixed(n: usize) -> (Expr, Num) {
        let m = 2usize.pow(n as u32);
        tree_with(n, &|| triangular(m))
    }
}

fn bench_expr_eval(c: &mut Criterion) {
    #![allow(clippy::type_complexity)]

    let implementations: [(&str, fn(&Expr) -> Num); 5] = [
        ("recursive", Expr::eval_recursive),
        ("trampolined", Expr::eval_trampolined),
        ("trampolined_opt", Expr::eval_trampolined_opt),
        ("iterative_cps", Expr::eval_iterative_cps),
        ("iterative_rpn", Expr::eval_iterative_rpn),
    ];

    let (simple, simple_eval) = examples::simple();
    for (_, impl_func) in implementations {
        assert_eq!(impl_func(&simple), simple_eval);
    }

    let cases: [(&str, fn(usize) -> (Expr, Num), usize); 4] = [
        ("triangular_{size}", examples::triangular, 200_000),
        ("power_{size}", examples::power, 200_000),
        ("complete_{size}", examples::complete, 19),
        ("mixed_{size}", examples::mixed, 9),
    ];

    let mut group = c.benchmark_group(format!("expr_{}", std::any::type_name::<Num>()));
    for (case_name, case_func, case_size) in cases {
        let case_name = &case_name.replace("{size}", &case_size.to_string());
        let (expr1, expr1_eval) = case_func(case_size);
        let (expr2, expr2_eval) = case_func((2 * (case_size + 1) - 2) / 2);

        assert_eq!(expr1.eval_recursive(), expr1_eval);
        stack_safe::with_stack_size(10 * 1024, move || {
            let expr = case_func(case_size).0;
            assert_eq!(expr.eval_trampolined(), expr1_eval);
            assert_eq!(expr.eval_trampolined_opt(), expr1_eval);
            assert_eq!(expr.eval_iterative_cps(), expr1_eval);
        })
        .unwrap();

        let exprs = (expr1, expr2);
        for (impl_name, impl_func) in implementations {
            group.bench_with_input(
                BenchmarkId::new(impl_name, case_name),
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
