// results & timings:
// A(3, 12) =  32765,   1.3 sec, recursive
// A(3, 12) =  32765,   6.3 sec, stack_safe
// A(3, 13) =  65533,   4.9 sec, recursive
// A(3, 13) =  65533,  23.6 sec, stack_safe
// A(3, 14) = 131069,  20.9 sec, recursive
// A(3, 14) = 131069,  99.3 sec, stack_safe
// ---- this is the ceiling for the recursive version
// A(3, 15) = 262141,  6:43 min, stack_safe
// A(3, 16) = 524285, 27:33 min, stack_safe
#![feature(generators, generator_trait)]
mod ackermann {
    #[allow(dead_code)]
    pub fn recursive(m: u64, n: u64) -> u64 {
        if m == 0 {
            n + 1
        } else if n == 0 {
            recursive(m - 1, 1)
        } else {
            recursive(m - 1, recursive(m, n - 1))
        }
    }

    #[allow(dead_code)]
    pub fn stack_safe(m: u64, n: u64) -> u64 {
        stack_safe::recurse(|(m, n): (u64, u64)| {
            move |_: u64| {
                if m == 0 {
                    n + 1
                } else if n == 0 {
                    yield (m - 1, 1)
                } else {
                    let k = yield (m, n - 1);
                    yield (m - 1, k)
                }
            }
        })((m, n))
    }
}

fn main() {
    println!("{}", ackermann::stack_safe(3, 12));
}
