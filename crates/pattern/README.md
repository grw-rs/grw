# grw_pattern

Token grammar for the `grw` pattern macros. It turns the node and edge arguments of `search!`,
`pattern!` and `modify!` clusters into rewritten calls: symbolic node names (`N(a)`), value patterns
(`N(a: Shape::Circle(_))`), pins (`X(c = id)`), key predicates (`N(a: key(IDX, expr))`,
`key_in(IDX, [..])`) and `with X(name = expr)` clauses for stored patterns.

It is a pure `syn`/`proc-macro2`/`quote` crate with no dependency on `grw`; `grw_derive` uses it
to expand the macros. You normally do not depend on it directly.

A rebuilt group carries the span of its original delimiters; proc-macro2 has no API to set the
open and close delimiter spans separately, so a span on either one alone is not available.

License: MIT OR Apache-2.0.
