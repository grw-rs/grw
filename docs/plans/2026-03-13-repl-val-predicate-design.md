# REPL Value Predicate Design

Scope: `GrwLayout` derive, `syn`-based closure parser, AST interpreter.
Out of scope: Cranelift JIT backend, `dot::parse` integration, enum support.

## Crates

```
grw/              existing library
  src/layout/     new module: Val trait, FieldInfo, FieldType
grw_derive/       new proc-macro crate: #[derive(Val)]
grw_repl/         new crate: closure parser + AST interpreter
  parse/          syn → Expr AST lowering
  interp/         AST interpreter using Val field info
```

Dependencies:
- `grw_repl` depends on `grw` (for `layout::Val`, graph types)
- `grw_derive` has no dependency on `grw` (generates code referencing `grw::layout::Val` by path)
- `grw` re-exports `grw_derive::Val` derive macro

## `grw::layout` module

```rust
pub enum FieldType {
    Bool,
    I8, I16, I32, I64,
    U8, U16, U32, U64,
    F32, F64,
    String,
    Struct(fn() -> &'static [FieldInfo]),
}

pub struct FieldInfo {
    pub name: &'static str,
    pub ty: FieldType,
    pub offset: usize,
}

pub trait Val {
    fn fields() -> &'static [FieldInfo];
    fn size() -> usize;
    fn align() -> usize;
}
```

Usage:
```rust
use grw::layout::Val;

#[derive(Val)]
struct Cargo {
    weight: f64,
    fragile: bool,
}
```

Derive macro uses `std::mem::offset_of!` (stable since Rust 1.77) for offsets.
Nested structs that also derive `Val` recurse into inner type's `Val::fields()`.

Supported field types (v1):
- Primitives: `bool`, `i8`/`i16`/`i32`/`i64`, `u8`/`u16`/`u32`/`u64`, `f32`/`f64`
- `String`
- Nested structs deriving `Val`

Excluded from v1: enums, `Option`, `Vec`, `HashMap`, references, `Box`, `Arc`.

## Closure parser (`grw_repl::parse`)

Input: string contents of `.test(...)`, e.g. `"|nv| nv.weight > 100.0 && !nv.fragile"`.

Uses `syn` to parse as a Rust closure expression, then lowers syn's AST to:

```rust
pub struct Closure {
    pub param: String,
    pub body: Expr,
}

pub enum Expr {
    Field { base: String, path: Vec<String> },
    LitInt(i64),
    LitFloat(f64),
    LitBool(bool),
    LitStr(String),
    BinOp { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    UnaryNot(Box<Expr>),
}

pub enum BinOp {
    Add, Sub, Mul, Div, Rem,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
}
```

Supported Rust syntax subset:
- `|param|` binding
- Field access: `param.field`, `param.field.nested`
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Boolean: `&&`, `||`, `!`
- Literals: integers, floats, bools, string literals
- Parentheses for grouping

Rejected with clear errors: `let` bindings, `if`/`match`, closures-in-closures, trait methods, type annotations, references/borrows.

## Interpreter (`grw_repl::interp`)

Runtime value type:
```rust
pub enum Value {
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Str(String),
}
```

Core evaluation:
```rust
pub fn eval(expr: &Expr, layout: &[FieldInfo], ptr: *const u8) -> Value
```

- Field access: lookup name in layout, read at `ptr + offset` using field type
- Nested fields: follow `Struct` variant recursively
- Binary ops: evaluate both sides, type-match (`Float + Float -> Float`, `Int > Int -> Bool`)
- Type mismatches at eval time: panic (bug in validation layer)

REPL-facing API:
```rust
pub fn compile_predicate<NV: Val>(
    src: &str,
) -> Result<Box<dyn Fn(&NV) -> bool>, PredError>
```

Parses, validates field names/types against `NV::fields()`, returns a closure.
The returned closure is infallible (all errors caught at parse/validation time, except bool return check).

## Error handling

Parse-time errors (returned in `Result`):
- `syn` parse failure: syntax error with span
- Unsupported syntax: "unsupported expression: match arms not available in REPL"
- Unknown field: "unknown field `foo` on type Cargo, available: weight, fragile, category"
- Nested field on non-struct: "`weight` is f64, cannot access `.x` on it"

Eval-time panics (bugs, not user errors):
- Type mismatch in binary op
- Division by zero

## Performance

- Parse + validate: one-time cost, ~microseconds (syn parsing)
- Eval per candidate: ~50-150ns (pointer arithmetic, no allocations)
- Acceptable for interactive use: 150ns * 10,000 nodes = 1.5ms

## Future work

- Cranelift JIT backend (~1-5ns/eval, desktop only)
- `dot::parse` integration (wire `.test()` bodies through this pipeline)
- Enum support in `Val` derive
- Method calls on primitives (`.abs()`, `.len()`)
- WASM target uses interpreter (no JIT in browsers)
