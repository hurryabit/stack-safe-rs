#![feature(destructuring_assignment, generators, generator_trait, step_trait)]
use std::mem;
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
        let mut gen = f(arg);
        let mut res = Res::default();

        loop {
            match Pin::new(&mut gen).resume(res) {
                GeneratorState::Yielded(arg) => {
                    stack.push(gen);
                    gen = f(arg);
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

pub fn recurse_st<Arg, Res, St, Gen>(f: impl Fn(Arg) -> Gen) -> impl Fn(Arg, &mut St) -> Res
where
    Res: Default,
    St: Default,
    Gen: Generator<(Res, St), Yield = (Arg, St), Return = (Res, St)> + Unpin,
{
    move |arg: Arg, st: &mut St| {
        let mut stack = Vec::new();
        let mut gen = f(arg);
        let mut res = Res::default();
        let mut st_def = St::default();

        loop {
            match Pin::new(&mut gen).resume((res, mem::replace(st, st_def))) {
                GeneratorState::Yielded((arg, st1)) => {
                    st_def = mem::replace(st, st1);
                    stack.push(gen);
                    gen = f(arg);
                    res = Res::default();
                }
                GeneratorState::Complete((res1, st1)) => {
                    st_def = mem::replace(st, st1);
                    match stack.pop() {
                        None => return res1,
                        Some(top) => {
                            gen = top;
                            res = res1;
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
