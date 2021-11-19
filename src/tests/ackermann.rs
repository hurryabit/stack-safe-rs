use crate::trampoline;

fn recursive(m: u64, n: u64) -> u64 {
    if m == 0 {
        n + 1
    } else if n == 0 {
        recursive(m - 1, 1)
    } else {
        recursive(m - 1, recursive(m, n - 1))
    }
}

fn stack_safe(m: u64, n: u64) -> u64 {
    trampoline(|(m, n): (u64, u64)| {
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


#[test]
#[should_panic]
#[ignore = "stack overflow is not an unwinding panic"]
fn recursive_is_unsafe() {
    assert_eq!(recursive(3, 12), 32765);
}

#[test]
fn stack_safe_is_safe() {
    assert_eq!(stack_safe(3, 10), 8189);
}
