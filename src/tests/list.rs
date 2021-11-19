use std::ops::Range;

use crate::{trampoline, with_stack_size};

enum List<T> {
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
    fn len_recursive(&self) -> usize {
        match self {
            Self::Nil => 0,
            Self::Cons { head: _, tail } => 1 + tail.len_recursive(),
        }
    }

    fn len_stack_safe(&self) -> usize {
        trampoline(|list: &List<T>| {
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

const LARGE: usize = 10_000;

#[test]
#[ignore = "stack overflow is not an unwinding panic"]
fn len_recursive_is_unsafe() {
    let result = with_stack_size(10 * 1024, || List::from(0..LARGE).len_recursive());
    assert!(result.is_err());
}

#[test]
fn len_stack_safe_is_safe() {
    let result = with_stack_size(1024, || List::from(0..LARGE).len_stack_safe());
    assert_eq!(result.unwrap(), LARGE);
}
