#![feature(generators, generator_trait, step_trait)]
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

mod list {
    use std::ops::Range;

    pub enum List<T> {
        Nil,
        Cons { head: T, tail: Box<List<T>> },
    }

    impl<T> Drop for List<T> {
        fn drop(&mut self) {
            if let Self::Cons { .. } = self {
                let mut list = std::mem::replace(self, List::Nil);
                while let Self::Cons { head, tail } = &mut list {
                    let next = std::mem::replace(tail.as_mut(), Self::Nil);
                    unsafe {
                        std::ptr::drop_in_place(head as *mut T);
                        std::ptr::drop_in_place(tail as *mut Box<List<T>>);
                    }
                    std::mem::forget::<List<T>>(list);
                    list = next;
                }
                std::mem::forget::<List<T>>(list);
            }
        }
    }

    impl<T> List<T> {
        pub fn len_recursive(&self) -> usize {
            match self {
                Self::Nil => 0,
                Self::Cons { head: _, tail } => 1 + tail.len_recursive(),
            }
        }

        pub fn len_stack_safe(&self) -> usize {
            stack_safe::recurse(|list: &List<T>| {
                move |_: usize| match list {
                    Self::Nil => 0,
                    Self::Cons { head: _, tail } => {
                        let tail_len = yield tail.as_ref();
                        1 + tail_len
                    }
                }
            })(self)
        }
    }

    impl<T: std::iter::Step> From<Range<T>> for List<T> {
        fn from(range: Range<T>) -> Self {
            let mut result = Self::Nil;
            for value in range.rev() {
                result = Self::Cons {
                    head: value,
                    tail: Box::new(result),
                }
            }
            result
        }
    }
}

pub fn bench_list_len(c: &mut Criterion) {
    use list::*;

    let cases: [(&str, usize); 1] = [("L_{size}", 1_000_000)];

    let mut group = c.benchmark_group("list_len");
    for (label, size) in cases {
        let label = label.replace("{size}", &size.to_string());
        let list = List::from(0..size);

        assert_eq!(list.len_recursive(), size);
        assert_eq!(
            stack_safe::with_stack_size(1024, move || List::from(0..size).len_stack_safe())
                .unwrap(),
            size,
        );

        group.bench_with_input(BenchmarkId::new("recursive", &label), &list, |b, list| {
            b.iter(|| {
                assert_eq!(list.len_recursive(), size);
            })
        });
        group.bench_with_input(BenchmarkId::new("stack_safe", &label), &list, |b, list| {
            b.iter(|| {
                assert_eq!(list.len_stack_safe(), size);
            })
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(2))
        .sample_size(20)
        .configure_from_args();
    targets = bench_list_len
}
criterion_main!(benches);
