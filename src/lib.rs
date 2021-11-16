#![feature(destructuring_assignment, generators, generator_trait, step_trait)]
use std::ops::{Generator, GeneratorState};
use std::pin::Pin;
use std::thread;

pub fn recurse<Arg, Res, Gen>(f: impl Fn(Arg) -> Gen) -> impl Fn(Arg) -> Res
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

pub struct Call<T> {
    arg: T,
    is_tail: bool,
}

impl<T> Call<T> {
    pub fn normal(arg: T) -> Self {
        Self {
            arg,
            is_tail: false,
        }
    }

    pub fn tail(arg: T) -> Self {
        Self { arg, is_tail: true }
    }
}

pub fn recurse_tco<Arg, Res, Gen>(f: impl Fn(Arg) -> Gen) -> impl Fn(Arg) -> Res
where
    Res: Default,
    Gen: Generator<Res, Yield = Call<Arg>, Return = Res> + Unpin,
{
    move |arg: Arg| {
        let mut stack = Vec::new();
        let mut gen = f(arg);
        let mut res = Res::default();

        loop {
            match Pin::new(&mut gen).resume(res) {
                GeneratorState::Yielded(call) => {
                    if !call.is_tail {
                        stack.push(gen);
                    }
                    gen = f(call.arg);
                    res = Res::default();
                }
                GeneratorState::Complete(res1) => match stack.pop() {
                    None => return res1,
                    Some(top) => {
                        gen = top;
                        res = res1;
                    }
                },
            }
        }
    }
}

pub fn recurse_mut<'a, Arg, MutArg, Res, Gen>(
    f: impl Fn(Arg) -> Gen,
) -> impl Fn(Arg, &'a mut MutArg) -> Res
where
    MutArg: 'a,
    Res: Default,
    Gen: Generator<
            (Res, &'a mut MutArg),
            Yield = (Arg, &'a mut MutArg),
            Return = (Res, &'a mut MutArg),
        > + Unpin,
{
    move |arg: Arg, mut mut_arg: &mut MutArg| {
        let mut stack = Vec::new();
        let mut gen = f(arg);
        let mut res = Res::default();

        loop {
            match Pin::new(&mut gen).resume((res, mut_arg)) {
                GeneratorState::Yielded((arg, new_mut_arg)) => {
                    mut_arg = new_mut_arg;
                    stack.push(gen);
                    gen = f(arg);
                    res = Res::default();
                }
                GeneratorState::Complete((new_res, new_mut_arg)) => {
                    mut_arg = new_mut_arg;
                    match stack.pop() {
                        None => return new_res,
                        Some(new_gen) => {
                            gen = new_gen;
                            res = new_res;
                        }
                    }
                }
            }
        }
    }
}

pub fn with_stack_size<T, F>(size: usize, f: F) -> thread::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    std::thread::Builder::new()
        .stack_size(size)
        .spawn(f)
        .unwrap()
        .join()
}

#[cfg(test)]
mod tests;
