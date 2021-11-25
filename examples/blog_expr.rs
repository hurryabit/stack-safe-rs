#![feature(generators, generator_trait)]
#![allow(
    clippy::borrowed_box,
    clippy::needless_return,
    clippy::while_let_on_iterator
)]
use stack_safe::trampoline;
use std::mem::MaybeUninit;
use std::ops::{Generator, GeneratorState};
use std::pin::Pin;

#[derive(Debug)]
pub enum Exp {
    Num(f64),
    Add(Vec<Exp>),
    Mul(Box<Exp>, Box<Exp>),
}

impl Exp {
    fn eval_recursive(&self) -> f64 {
        match self {
            Exp::Num(val) => *val,
            Exp::Add(exps) => {
                let mut sum = 0.0;
                for exp in exps {
                    let val = exp.eval_recursive();
                    sum += val;
                }
                sum
            }
            Exp::Mul(exp1, exp2) => {
                let val1 = exp1.eval_recursive();
                let val2 = exp2.eval_recursive();
                val1 * val2
            }
        }
    }
}

impl Exp {
    fn eval_generator(&self) -> impl Generator<f64, Yield = &Self, Return = f64> {
        move |_| match self {
            Exp::Num(val) => {
                return *val;
            }
            Exp::Add(exps) => {
                let mut sum = 0.0;
                let mut iter = exps.iter();
                while let Some(exp) = iter.next() {
                    let val = yield exp; // Gen::Add
                    sum += val;
                }
                return sum;
            }
            Exp::Mul(exp1, exp2) => {
                let val1 = yield exp1; // Gen::Mul1
                let val2 = yield exp2; // Gen::Mul2
                return val1 * val2;
            }
        }
    }
}

pub enum SimpleGen<'a> {
    Unresumed {
        init: &'a Exp,
    },
    Returned,
    Add {
        sum: f64,
        iter: std::slice::Iter<'a, Exp>,
    },
    Mul1 {
        exp2: &'a Box<Exp>,
    },
    Mul2 {
        val1: f64,
    },
}

impl<'a> SimpleGen<'a> {
    pub fn new(init: &'a Exp) -> Self {
        SimpleGen::Unresumed { init }
    }
}

impl<'a> Generator<f64> for SimpleGen<'a> {
    type Yield = &'a Exp;
    type Return = f64;

    fn resume(self: Pin<&mut Self>, arg: f64) -> GeneratorState<Self::Yield, Self::Return> {
        let this = self.get_mut();
        match this {
            SimpleGen::Unresumed { init } => match init {
                Exp::Num(val) => {
                    *this = SimpleGen::Returned;
                    GeneratorState::Complete(*val)
                }
                Exp::Add(exps) => {
                    let sum = 0.0;
                    let mut iter = exps.iter();
                    if let Some(exp) = iter.next() {
                        *this = SimpleGen::Add { sum, iter };
                        GeneratorState::Yielded(exp)
                    } else {
                        *this = SimpleGen::Returned;
                        GeneratorState::Complete(sum)
                    }
                }
                Exp::Mul(exp1, exp2) => {
                    *this = SimpleGen::Mul1 { exp2 };
                    GeneratorState::Yielded(exp1)
                }
            },
            SimpleGen::Returned => panic!("resuming returned generator"),
            SimpleGen::Add { sum, iter } => {
                let val = arg;
                *sum += val;
                if let Some(exp) = iter.next() {
                    // We don't need to change `this` here because we have
                    // updated `sum` and `iter` in place.
                    GeneratorState::Yielded(exp)
                } else {
                    let sum = *sum;
                    *this = SimpleGen::Returned;
                    GeneratorState::Complete(sum)
                }
            }
            SimpleGen::Mul1 { exp2 } => {
                let val1 = arg;
                let exp2 = *exp2;
                *this = SimpleGen::Mul2 { val1 };
                GeneratorState::Yielded(exp2)
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

union SumOrExp2<'a> {
    sum: f64,
    exp2: &'a Box<Exp>,
}

pub struct ActualGen<'a> {
    discriminator: ActualGenDiscriminator,
    init: &'a Exp,
    sum_or_exp2: MaybeUninit<SumOrExp2<'a>>,
    val1: MaybeUninit<f64>,
    iter: MaybeUninit<std::slice::Iter<'a, Exp>>,
}

impl<'a> ActualGen<'a> {
    pub fn new(init: &'a Exp) -> Self {
        ActualGen {
            discriminator: ActualGenDiscriminator::Unresumed,
            init,
            val1: MaybeUninit::uninit(),
            sum_or_exp2: MaybeUninit::uninit(),
            iter: MaybeUninit::uninit(),
        }
    }
}

impl<'a> Generator<f64> for ActualGen<'a> {
    type Yield = &'a Exp;
    type Return = f64;

    fn resume(self: Pin<&mut Self>, arg: f64) -> GeneratorState<Self::Yield, Self::Return> {
        unsafe {
            let this = self.get_mut();
            match this.discriminator {
                ActualGenDiscriminator::Unresumed => match this.init {
                    Exp::Num(val) => {
                        this.discriminator = ActualGenDiscriminator::Returned;
                        GeneratorState::Complete(*val)
                    }
                    Exp::Add(exps) => {
                        this.sum_or_exp2 = MaybeUninit::new(SumOrExp2 { sum: 0.0 });
                        this.iter = MaybeUninit::new(exps.iter());
                        if let Some(exp) = this.iter.assume_init_mut().next() {
                            this.discriminator = ActualGenDiscriminator::Add;

                            GeneratorState::Yielded(exp)
                        } else {
                            this.discriminator = ActualGenDiscriminator::Returned;
                            GeneratorState::Complete(this.sum_or_exp2.assume_init_ref().sum)
                        }
                    }
                    Exp::Mul(exp1, exp2) => {
                        this.discriminator = ActualGenDiscriminator::Mul1;
                        this.sum_or_exp2 = MaybeUninit::new(SumOrExp2 { exp2 });
                        GeneratorState::Yielded(exp1)
                    }
                },
                ActualGenDiscriminator::Returned => panic!("resuming returned generator"),
                ActualGenDiscriminator::Panicked => panic!("resuming panicked generator"),
                ActualGenDiscriminator::Add => {
                    let val = arg;
                    this.sum_or_exp2.assume_init_mut().sum += val;
                    if let Some(exp) = this.iter.assume_init_mut().next() {
                        GeneratorState::Yielded(exp)
                    } else {
                        this.discriminator = ActualGenDiscriminator::Returned;
                        GeneratorState::Complete(this.sum_or_exp2.assume_init_ref().sum)
                    }
                }
                ActualGenDiscriminator::Mul1 => {
                    let val1 = arg;
                    this.discriminator = ActualGenDiscriminator::Mul2;
                    this.val1 = MaybeUninit::new(val1);
                    GeneratorState::Yielded(this.sum_or_exp2.assume_init_ref().exp2)
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
    let exp = {
        use Exp::*;
        Mul(Box::new(Num(2.0)), Box::new(Add(vec![Num(3.0)])))
    };
    dbg!(std::mem::size_of_val(&exp.eval_generator()));
    dbg!(std::mem::size_of_val(&SimpleGen::new(&exp)));

    dbg!(exp.eval_recursive());
    dbg!(trampoline(Exp::eval_generator)(&exp));
    dbg!(trampoline(SimpleGen::new)(&exp));
    dbg!(trampoline(ActualGen::new)(&exp));
}
