// This file contains the code from the blog post
// https://hurryabit.github.io/blog/stack-safety-for-free/
#![feature(generators, generator_trait)]
use std::ops::{Generator, GeneratorState};
use std::pin::Pin;

fn triangular(n: u64) -> u64 {
    if n == 0 {
        0
    } else {
        n + triangular(n - 1)
    }
}

fn triangular_safe(n: u64) -> u64 {
    recurse(|n| move |_| {
        if n == 0 {
            0
        } else {
            n + yield (n - 1)
        }
    })(n)
}

fn recurse<Arg, Res, Gen>(
    f: impl Fn(Arg) -> Gen
) -> impl Fn(Arg) -> Res
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
                GeneratorState::Complete(real_res) => {
                    match stack.pop() {
                        None => return real_res,
                        Some(top) => {
                            current = top;
                            res = real_res;
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    const LARGE: u64 = 1_000_000;

    assert_eq!(triangular_safe(LARGE), LARGE * (LARGE + 1) / 2);
    println!("`triangular_safe` has not overflowed its stack.");

    println!("`triangular` will overflow its stack soon...");
    assert_eq!(triangular(LARGE), LARGE * (LARGE + 1) / 2);
}
