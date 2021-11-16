/*
 results & timings:

+─────────────+─────────+───────+────────────+─────────────+─────────+────────────+────────+
|             | result  | loop  | recursive  | manual-tco  | manual  | yield-tco  | yield  |
+─────────────+─────────+───────+────────────+─────────────+─────────+────────────+────────+
| "A(3, 12)"  | 32765   |       | 1.3        |             |         |            | 6.3    |
| "A(3, 13)"  | 65533   | 2.8   | 4.3        | 4.4         | 6.0     | 5.3        | 18.7   |
| "A(3, 14)"  | 131069  |       | 20.9       |             |         |            | 99.3   |
| "A(3, 15)"  | 262141  |       | SO         |             |         |            | 403    |
| "A(3, 16)"  | 524285  |       | SO         |             |         |            | 1650   |
+─────────────+─────────+───────+────────────+─────────────+─────────+────────────+────────+
 */

#![feature(generators, generator_trait)]

mod ackermann {
    use stack_safe::{recurse, recurse_tco, Call};

    pub fn recursive(m: u64, n: u64) -> u64 {
        if m == 0 {
            n + 1
        } else if n == 0 {
            recursive(m - 1, 1)
        } else {
            recursive(m - 1, recursive(m, n - 1))
        }
    }

    pub fn r#loop(mut m: u64, mut n: u64) -> u64 {
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

    pub fn r#yield(m: u64, n: u64) -> u64 {
        recurse(|(m, n): (u64, u64)| {
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

    pub fn yield_tco(m: u64, n: u64) -> u64 {
        recurse_tco(|(m, n): (u64, u64)| {
            move |_: u64| {
                if m == 0 {
                    n + 1
                } else if n == 0 {
                    yield Call::tail((m - 1, 1))
                } else {
                    let k = yield Call::normal((m, n - 1));
                    yield Call::tail((m - 1, k))
                }
            }
        })((m, n))
    }

    pub mod manual {
        use std::ops::{Generator, GeneratorState};
        use std::pin::Pin;

        pub enum Kont {
            A { m: u64, n: u64 },
            B,
            C { m: u64 },
            D,
        }

        impl Kont {
            pub fn init(m: u64, n: u64) -> Self {
                Self::A { m, n }
            }
        }

        impl Generator<u64> for Kont {
            type Yield = (u64, u64);
            type Return = u64;

            fn resume(self: Pin<&mut Self>, r: u64) -> GeneratorState<Self::Yield, Self::Return> {
                match *self {
                    Self::A { m, n } => {
                        if m == 0 {
                            GeneratorState::Complete(n + 1)
                        } else if n == 0 {
                            *self.get_mut() = Self::B;
                            GeneratorState::Yielded((m - 1, 1))
                        } else {
                            *self.get_mut() = Self::C { m };
                            GeneratorState::Yielded((m, n - 1))
                        }
                    }
                    Self::B => GeneratorState::Complete(r),
                    Self::C { m } => {
                        *self.get_mut() = Self::D;
                        GeneratorState::Yielded((m - 1, r))
                    }
                    Self::D => GeneratorState::Complete(r),
                }
            }
        }
    }

    pub fn manual(m: u64, n: u64) -> u64 {
        recurse(|(m, n)| manual::Kont::init(m, n))((m, n))
    }

    pub mod manual_tco {
        use stack_safe::Call;
        use std::ops::{Generator, GeneratorState};
        use std::pin::Pin;

        pub enum Kont {
            A { m: u64, n: u64 },
            C { m: u64 },
        }

        impl Kont {
            pub fn init(m: u64, n: u64) -> Self {
                Self::A { m, n }
            }
        }

        impl Generator<u64> for Kont {
            type Yield = Call<(u64, u64)>;
            type Return = u64;

            fn resume(self: Pin<&mut Self>, r: u64) -> GeneratorState<Self::Yield, Self::Return> {
                match *self {
                    Self::A { m, n } => {
                        if m == 0 {
                            GeneratorState::Complete(n + 1)
                        } else if n == 0 {
                            GeneratorState::Yielded(Call::tail((m - 1, 1)))
                        } else {
                            *self.get_mut() = Self::C { m };
                            GeneratorState::Yielded(Call::normal((m, n - 1)))
                        }
                    }
                    Self::C { m } => GeneratorState::Yielded(Call::tail((m - 1, r))),
                }
            }
        }
    }

    pub fn manual_tco(m: u64, n: u64) -> u64 {
        recurse_tco(|(m, n)| manual_tco::Kont::init(m, n))((m, n))
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
                    "loop",
                    "yield",
                    "yield-tco",
                    "manual",
                    "manual-tco",
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
        "loop" => ackermann::r#loop,
        "yield" => ackermann::r#yield,
        "yield-tco" => ackermann::yield_tco,
        "manual" => ackermann::manual,
        "manual-tco" => ackermann::manual_tco,
        _ => panic!("Impossible value for IMPL."),
    };
    let m = value_t!(matches.value_of("M"), u64).unwrap_or_else(|e| e.exit());
    let n = value_t!(matches.value_of("N"), u64).unwrap_or_else(|e| e.exit());

    println!("{}", implementation(m, n));
}
