#![feature(generators, generator_trait)]
#![allow(clippy::while_let_on_iterator, clippy::borrowed_box)]
use stack_safe::trampoline;
use std::mem::MaybeUninit;
use std::ops::{Generator, GeneratorState};
use std::pin::Pin;

#[derive(Debug)]
pub enum Expr {
    Num(f64),
    Add(Vec<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

impl Expr {
    fn eval_recursive(&self) -> f64 {
        match self {
            Expr::Num(num) => *num,
            Expr::Add(exprs) => {
                let mut sum = 0.0;
                for expr in exprs {
                    sum += expr.eval_recursive();
                }
                sum
            }
            Expr::Mul(expr1, expr2) => expr1.eval_recursive() * expr2.eval_recursive(),
        }
    }

    fn eval_generator(&self) -> impl Generator<f64, Yield = &Self, Return = f64> {
        move |_: f64| match self {
            Expr::Num(val) => *val,
            Expr::Add(exprs) => {
                let mut sum = 0.0;
                let mut iter = exprs.iter();
                while let Some(expr) = iter.next() {
                    let val = yield expr; // Gen::Add
                    sum += val;
                }
                sum
            }
            Expr::Mul(expr1, expr2) => {
                let val1 = yield expr1; // Gen::Mul1
                let val2 = yield expr2; // Gen::Mul2
                val1 * val2
            }
        }
    }
}

pub enum SimpleGen<'a> {
    Unresumed {
        init: &'a Expr,
    },
    Returned,
    Add {
        sum: f64,
        iter: std::slice::Iter<'a, Expr>,
    },
    Mul1 {
        expr2: &'a Box<Expr>,
    },
    Mul2 {
        val1: f64,
    },
}

impl<'a> SimpleGen<'a> {
    pub fn new(init: &'a Expr) -> Self {
        SimpleGen::Unresumed { init }
    }
}

impl<'a> Generator<f64> for SimpleGen<'a> {
    type Yield = &'a Expr;
    type Return = f64;

    fn resume(self: Pin<&mut Self>, arg: f64) -> GeneratorState<Self::Yield, Self::Return> {
        let this = self.get_mut();
        match this {
            SimpleGen::Unresumed { init } => match init {
                Expr::Num(val) => {
                    *this = SimpleGen::Returned;
                    GeneratorState::Complete(*val)
                }
                Expr::Add(exprs) => {
                    let sum = 0.0;
                    let mut iter = exprs.iter();
                    if let Some(expr) = iter.next() {
                        *this = SimpleGen::Add { sum, iter };
                        GeneratorState::Yielded(expr)
                    } else {
                        *this = SimpleGen::Returned;
                        GeneratorState::Complete(sum)
                    }
                }
                Expr::Mul(expr1, expr2) => {
                    *this = SimpleGen::Mul1 { expr2 };
                    GeneratorState::Yielded(expr1)
                }
            },
            SimpleGen::Returned => panic!("resuming returned generator"),
            SimpleGen::Add { sum, iter } => {
                let val = arg;
                *sum += val;
                if let Some(expr) = iter.next() {
                    // We don't need to change `this` here because we have
                    // updated `sum` and `iter` in place.
                    GeneratorState::Yielded(expr)
                } else {
                    let sum = *sum;
                    *this = SimpleGen::Returned;
                    GeneratorState::Complete(sum)
                }
            }
            SimpleGen::Mul1 { expr2 } => {
                let val1 = arg;
                let expr2 = *expr2;
                *this = SimpleGen::Mul2 { val1 };
                GeneratorState::Yielded(expr2)
            }
            SimpleGen::Mul2 { val1 } => {
                let val2 = arg;
                let val1 = *val1;
                *this = SimpleGen::Returned;
                GeneratorState::Complete(val1 * val2)
            }
        }
    }
}

pub enum ActualGenDiscriminator {
    Unresumed,
    Returned,
    Panicked,
    Add,
    Mul1,
    Mul2,
}

union SumOrExpr2<'a> {
    sum: f64,
    expr2: &'a Box<Expr>,
}

pub struct ActualGen<'a> {
    discriminator: ActualGenDiscriminator,
    init: &'a Expr,
    sum_or_expr2: MaybeUninit<SumOrExpr2<'a>>,
    val1: MaybeUninit<f64>,
    iter: MaybeUninit<std::slice::Iter<'a, Expr>>,
}

impl<'a> ActualGen<'a> {
    pub fn new(init: &'a Expr) -> Self {
        ActualGen {
            discriminator: ActualGenDiscriminator::Unresumed,
            init,
            val1: MaybeUninit::uninit(),
            sum_or_expr2: MaybeUninit::uninit(),
            iter: MaybeUninit::uninit(),
        }
    }
}

impl<'a> Generator<f64> for ActualGen<'a> {
    type Yield = &'a Expr;
    type Return = f64;

    fn resume(self: Pin<&mut Self>, arg: f64) -> GeneratorState<Self::Yield, Self::Return> {
        unsafe {
            let this = self.get_mut();
            match this.discriminator {
                ActualGenDiscriminator::Unresumed => match this.init {
                    Expr::Num(val) => {
                        this.discriminator = ActualGenDiscriminator::Returned;
                        GeneratorState::Complete(*val)
                    }
                    Expr::Add(exprs) => {
                        this.sum_or_expr2 = MaybeUninit::new(SumOrExpr2 { sum: 0.0 });
                        this.iter = MaybeUninit::new(exprs.iter());
                        if let Some(expr) = this.iter.assume_init_mut().next() {
                            this.discriminator = ActualGenDiscriminator::Add;

                            GeneratorState::Yielded(expr)
                        } else {
                            this.discriminator = ActualGenDiscriminator::Returned;
                            GeneratorState::Complete(this.sum_or_expr2.assume_init_ref().sum)
                        }
                    }
                    Expr::Mul(expr1, expr2) => {
                        this.discriminator = ActualGenDiscriminator::Mul1;
                        this.sum_or_expr2 = MaybeUninit::new(SumOrExpr2 { expr2 });
                        GeneratorState::Yielded(expr1)
                    }
                },
                ActualGenDiscriminator::Returned => panic!("resuming returned generator"),
                ActualGenDiscriminator::Panicked => panic!("resuming panicked generator"),
                ActualGenDiscriminator::Add => {
                    let val = arg;
                    this.sum_or_expr2.assume_init_mut().sum += val;
                    if let Some(expr) = this.iter.assume_init_mut().next() {
                        GeneratorState::Yielded(expr)
                    } else {
                        this.discriminator = ActualGenDiscriminator::Returned;
                        GeneratorState::Complete(this.sum_or_expr2.assume_init_ref().sum)
                    }
                }
                ActualGenDiscriminator::Mul1 => {
                    let val1 = arg;
                    this.discriminator = ActualGenDiscriminator::Mul2;
                    this.val1 = MaybeUninit::new(val1);
                    GeneratorState::Yielded(this.sum_or_expr2.assume_init_ref().expr2)
                }
                ActualGenDiscriminator::Mul2 => {
                    let val2 = arg;
                    this.discriminator = ActualGenDiscriminator::Returned;
                    GeneratorState::Complete(this.val1.assume_init_ref() * val2)
                }
            }
        }
    }
}

fn main() {
    let expr = {
        use Expr::*;
        Mul(Box::new(Num(2.0)), Box::new(Add(vec![Num(3.0)])))
    };
    dbg!(std::mem::size_of_val(&expr.eval_generator()));
    dbg!(std::mem::size_of_val(&SimpleGen::new(&expr)));

    dbg!(expr.eval_recursive());
    dbg!(trampoline(Expr::eval_generator)(&expr));
    dbg!(trampoline(SimpleGen::new)(&expr));
    dbg!(trampoline(ActualGen::new)(&expr));
}
