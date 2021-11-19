use crate::trampoline;

fn binomial_stack_safe(n: u64, k: u64) -> u64 {
    trampoline(|(n, k)| move |_| {
        if k == 0 || k == n {
            1
        } else {
            (yield (n - 1, k - 1)) + (yield (n - 1, k))
        }
    })((n, k))
}




#[test]
fn binomial_10_3() {
    assert_eq!(binomial_stack_safe(10, 3), 120);
}
