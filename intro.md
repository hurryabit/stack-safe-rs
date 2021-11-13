---
layout: post
title: Stack-safety for free?
author: Martin Huschenbett <@hurryabit>
date: 2021-11-13
---

# Stack-safety for free?

Dear Reader,

Imagine you have implemented an algorithm which contains a complex recursive
function, such as
[Tarjan's strongly connected components algorithm](https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm)
with its depth-first search, and your inputs have become so big that the
recursive calls blow the stack. Although there is a well-understood algorithm
that can transform any recursive function into an iterative implementation,
actually carrying out this transformation is quite tedious and error-prone.

In this Gist, I sketch a general technique for performing this transformation
with almost no code changes at all. This technique works in any programming
language which supports
[generators](https://en.wikipedia.org/wiki/Generator_(computer_programming))
or [coroutines](https://en.wikipedia.org/wiki/Coroutine).
I provide a model implementation in Rust nightly.

The technique consists of two steps only:

1. replace every recursive call with a `yield`, and

2. wrap the resulting generator in the higher-order function `recurse`.

The result of `recurse` will then be a function that behaves like the original
recursive function and is stack-safe at the same time. The code below
illustrates how to transform the recursive function `triangular`, which
computes triangular numbers, into the equivalent but stack-safe function
`triangular_safe`. Although `triangular` is surely not the sort of function
where one would resort to general techniques, its simplicity allows us to
focus on the technique itself rather than being distracted by the complexity
of the recursive function.

The code below also provides an implementation of the higher-order function
`recurse`. Rougly speaking, `recurse` maintains a stack of partially run
generators that resembles the call stack of the original recursive version.
The `main` function demonstrates how the first function blows the stack when
given a large input while the second does not.

The key insight behind the technique is the fact that the generic algorithm
for rewriting recursion into a loop performs almost exactly the same code
transformation that a compiler does when desugaring generators. Thus, instead
of rewriting our code by hand, we can simply let the compiler do it for us.

The implementation below does neither support functions with multiple
arguments nor mutual recursion. Both features are conceptually straightforward
to add; the former by packing multiple arguments into a single tuple, the
latter by yielding a pair of the function to call and the argument to call it
with.

In the case of Rust, it is not yet clear how this technique interacts with
the ownership system in the presence of more complex types and captured
variables. I will keep playing with the idea to find out. Another important
question concerns performance. I still have to conduct extensive benchmarks to
see how the 'yield & recurse' version performs in comparison to both the
original function as well as a hand-written iterative version of some complex
recursive function. The depth-first search in Tarjan's aforementioned
algorithm seems to be a good candidate for such benchmarks.

My ultimate vision for this little project is to create a Rust package with a
procedural attribute macro `#[stack_safe]` that we can slap on any recursive
function to make it stack-safe using this technique or a variation of it.
If I, or we, manage to achieve good performance with this approach, I would be
tempted to describe the outcome as "stack-safety for free".

If you have read so far, I would like to thank you for taking the time. If
you have any thoughts on this idea, please leave a comment and I will do my
best to get back to you.

Thank you,

Martin.
