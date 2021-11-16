// results & timings:
// A(3, 12) =  32765,   1.3 sec, recursive
// A(3, 12) =  32765,   6.3 sec, stack-safe
// A(3, 13) =  65533,   2.8 sec, manual-loop
// A(3, 13) =  65533,   4.3 sec, recursive
// A(3, 13) =  65533,   6.1 sec, systematic-tco-loop
// A(3, 13) =  65533,   8.2 sec, systematic-loop
// A(3, 13) =  65533,  21.7 sec, stack-safe
// A(3, 14) = 131069,  20.9 sec, recursive
// A(3, 14) = 131069,  99.3 sec, stack-safe
// ---- this is the ceiling for the recursive version
// A(3, 15) = 262141,  6:43 min, stack-safe
// A(3, 16) = 524285, 27:33 min, stack-safe
#![feature(generators, generator_trait)]

mod ackermann {
    pub fn recursive(m: u64, n: u64) -> u64 {
        if m == 0 {
            n + 1
        } else if n == 0 {
            recursive(m - 1, 1)
        } else {
            recursive(m - 1, recursive(m, n - 1))
        }
    }

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

    pub fn manual_loop(mut m: u64, mut n: u64) -> u64 {
        let mut stack = Vec::new();
        while !(m == 0 && stack.is_empty()) {
            if m == 0 {
                m = stack.pop().unwrap();
                n += 1;
            } else if n == 0 {
                m -= 1;
                n = 1;
            } else {
                stack.push(m - 1);
                n -= 1;
            }
        }
        n + 1
    }

    pub mod systematic {
        enum Kont {
            A { m: u64, n: u64 },
            B,
            C { m: u64 },
            D,
        }

        enum Outcome {
            Yield(u64, u64, Kont),
            Return(u64),
        }

        impl Kont {
            pub fn init(m: u64, n: u64) -> Self {
                Self::A { m, n }
            }

            pub fn resume(self, r: u64) -> Outcome {
                match self {
                    Self::A { m, n } => {
                        if m == 0 {
                            Outcome::Return(n + 1)
                        } else if n == 0 {
                            Outcome::Yield(m - 1, 1, Self::B)
                        } else {
                            Outcome::Yield(m, n - 1, Self::C { m })
                        }
                    }
                    Self::B => Outcome::Return(r),
                    Self::C { m } => Outcome::Yield(m - 1, r, Self::D),
                    Self::D => Outcome::Return(r),
                }
            }
        }

        pub fn systematic_loop(m: u64, n: u64) -> u64 {
            let mut stack = vec![Kont::init(m, n)];
            let mut res = 0;
            while let Some(kont) = stack.pop() {
                match kont.resume(res) {
                    Outcome::Yield(m, n, kont) => {
                        stack.push(kont);
                        stack.push(Kont::init(m, n));
                        res = 0;
                    }
                    Outcome::Return(r) => {
                        res = r;
                    }
                }
            }
            res
        }
    }

    pub mod systematic_tco {
        enum Kont {
            A { m: u64, n: u64 },
            C { m: u64 },
        }

        enum Outcome {
            Yield(u64, u64, Kont),
            TailCall(u64, u64),
            Return(u64),
        }

        impl Kont {
            pub fn init(m: u64, n: u64) -> Self {
                Self::A { m, n }
            }

            pub fn resume(self, r: u64) -> Outcome {
                match self {
                    Self::A { m, n } => {
                        if m == 0 {
                            Outcome::Return(n + 1)
                        } else if n == 0 {
                            Outcome::TailCall(m - 1, 1)
                        } else {
                            Outcome::Yield(m, n - 1, Self::C { m })
                        }
                    }
                    Self::C { m } => Outcome::TailCall(m - 1, r),
                }
            }
        }

        pub fn systematic_loop(m: u64, n: u64) -> u64 {
            let mut stack = vec![Kont::init(m, n)];
            let mut res = 0;
            while let Some(kont) = stack.pop() {
                match kont.resume(res) {
                    Outcome::Yield(m, n, kont) => {
                        stack.push(kont);
                        stack.push(Kont::init(m, n));
                        res = 0;
                    }
                    Outcome::TailCall(m, n) => {
                        stack.push(Kont::init(m, n));
                        res = 0;
                    }
                    Outcome::Return(r) => {
                        res = r;
                    }
                }
            }
            res
        }
    }
}

fn validate_u64_arg(arg: String) -> Result<(), String> {
    match arg.parse::<u64>() {
        Ok(_) => Ok(()),
        Err(_) => Err(String::from("expected integer")),
    }
}

fn main() {
    use clap::{value_t, App, Arg};

    let matches = App::new("Ackermann computer")
        .version("0.0.1")
        .about("Computes the Ackermann function")
        .arg(
            Arg::with_name("IMPL")
                .help("implementation to use")
                .required(true)
                .possible_values(&[
                    "recursive",
                    "stack-safe",
                    "manual-loop",
                    "systematic-loop",
                    "systematic-tco-loop",
                ]),
        )
        .arg(
            Arg::with_name("M")
                .help("integer to pass as first argument")
                .required(true)
                .validator(validate_u64_arg),
        )
        .arg(
            Arg::with_name("N")
                .help("integer to pass as second argument")
                .required(true)
                .validator(validate_u64_arg),
        )
        .get_matches();

    let implementation = match matches.value_of("IMPL").unwrap() {
        "recursive" => ackermann::recursive,
        "stack-safe" => ackermann::stack_safe,
        "manual-loop" => ackermann::manual_loop,
        "systematic-loop" => ackermann::systematic::systematic_loop,
        "systematic-tco-loop" => ackermann::systematic_tco::systematic_loop,
        _ => panic!("Impossible value for IMPL."),
    };
    let m = value_t!(matches.value_of("M"), u64).unwrap_or_else(|e| e.exit());
    let n = value_t!(matches.value_of("N"), u64).unwrap_or_else(|e| e.exit());

    println!("{}", implementation(m, n));
}
