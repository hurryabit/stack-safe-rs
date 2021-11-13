#![feature(destructuring_assignment, generators, generator_trait)]
use std::mem;
use std::ops::{Generator, GeneratorState};
use std::pin::Pin;

pub mod tarjan;

pub fn recurse<Arg, Res, Gen>(f: impl Fn(Arg) -> Gen) -> impl Fn(Arg) -> Res
where
    Res: Default,
    Gen: Generator<Res, Yield = Arg, Return = Res> + Unpin,
{
    move |arg: Arg| {
        let mut stack = vec![f(arg)];
        let mut res = Res::default();

        while let Some(mut gen) = stack.pop() {
            match Pin::new(&mut gen).resume(res) {
                GeneratorState::Yielded(arg) => {
                    stack.push(gen);
                    stack.push(f(arg));
                    res = Res::default();
                }
                GeneratorState::Complete(res1) => {
                    res = res1;
                }
            }
        }

        res
    }
}

pub fn recurse_st<Arg, Res, St, Gen>(f: impl Fn(Arg) -> Gen) -> impl Fn(Arg, &mut St) -> Res
where
    Res: Default,
    St: Default,
    Gen: Generator<(Res, St), Yield = (Arg, St), Return = (Res, St)> + Unpin,
{
    move |arg: Arg, st: &mut St| {
        let mut stack = vec![f(arg)];
        let mut res = Res::default();
        let mut st_def = St::default();

        while let Some(mut gen) = stack.pop() {
            match Pin::new(&mut gen).resume((res, mem::replace(st, st_def))) {
                GeneratorState::Yielded((arg, st1)) => {
                    st_def = mem::replace(st, st1);
                    stack.push(gen);
                    stack.push(f(arg));
                    res = Res::default();
                }
                GeneratorState::Complete((res1, st1)) => {
                    res = res1;
                    st_def = mem::replace(st, st1);
                }
            }
        }

        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangular(n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            n + triangular(n - 1)
        }
    }

    fn triangular_safe(n: u64) -> u64 {
        recurse(|n: u64| {
            move |_: u64| {
                if n == 0 {
                    0
                } else {
                    n + yield (n - 1)
                }
            }
        })(n)
    }

    const LARGE: u64 = 1000;

    #[test]
    fn triangular_safe_is_safe() {
        let handle = std::thread::Builder::new()
            .stack_size(512)
            .spawn(|| triangular_safe(LARGE))
            .unwrap();
        assert_eq!(handle.join().unwrap(), LARGE * (LARGE + 1) / 2);
    }

    #[test]
    #[ignore = "stack overflow is not an unwinding panic"]
    fn triangular_is_unsafe() {
        let handler = std::thread::Builder::new()
            .stack_size(512)
            .spawn(|| triangular(LARGE))
            .unwrap();
        assert!(handler.join().is_err());
    }
}
