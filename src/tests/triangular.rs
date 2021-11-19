use crate::{trampoline, with_stack_size};

fn recursive(n: u64) -> u64 {
    if n == 0 {
        0
    } else {
        n + recursive(n - 1)
    }
}

fn stack_safe(n: u64) -> u64 {
    trampoline(|n: u64| {
        move |_: u64| {
            if n == 0 {
                0
            } else {
                n + yield (n - 1)
            }
        }
    })(n)
}

const LARGE: u64 = 10_000;

#[test]
#[ignore = "stack overflow is not an unwinding panic"]
fn recursive_is_unsafe() {
    let result = with_stack_size(10 * 1024, || recursive(LARGE));
    assert!(result.is_err());
}

#[test]
fn stack_safe_is_safe() {
    let result = with_stack_size(512, || stack_safe(LARGE));
    assert_eq!(result.unwrap(), LARGE * (LARGE + 1) / 2);
}
