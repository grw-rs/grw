# REPL Value Predicate Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable runtime evaluation of `.test()` predicates written as Rust closures, parsed by `syn` and interpreted against struct field layouts provided by a `#[derive(Val)]` macro.

**Architecture:** Three crates in a Cargo workspace — `grw` (existing, gains `layout` module with `Val` trait), `grw_derive` (proc-macro crate, `#[derive(Val)]`), `grw_repl` (closure parser + AST interpreter). `grw_derive` generates code referencing `grw::layout::Val` by path. `grw_repl` depends on `grw` for the trait and graph types.

**Tech Stack:** Rust 2024 edition, `syn` (for closure parsing), `proc-macro2`/`quote` (for derive macro), `std::mem::offset_of!` (stable since 1.77).

**Spec:** `docs/plans/2026-03-13-repl-val-predicate-design.md`

---

## File Structure

### grw (existing crate, modified)

| File | Action | Responsibility |
|------|--------|---------------|
| `Cargo.toml` | Modify | Convert to workspace root, add `grw_derive` dep |
| `src/lib.rs` | Modify | Add `pub mod layout;`, re-export derive macro |
| `src/layout/mod.rs` | Create | `Val` trait, `FieldInfo`, `FieldType` types |

### grw_derive (new proc-macro crate)

| File | Action | Responsibility |
|------|--------|---------------|
| `grw_derive/Cargo.toml` | Create | proc-macro crate with `syn`, `quote`, `proc-macro2` deps |
| `grw_derive/src/lib.rs` | Create | `#[derive(Val)]` proc-macro entry point |
| `grw_derive/src/val.rs` | Create | Derive expansion logic: field iteration, offset/type codegen |

### grw_repl (new crate)

| File | Action | Responsibility |
|------|--------|---------------|
| `grw_repl/Cargo.toml` | Create | Depends on `grw` and `syn` |
| `grw_repl/src/lib.rs` | Create | Module declarations, `compile_predicate` public API |
| `grw_repl/src/parse.rs` | Create | `syn`-based closure parser, lowers to `Expr` AST |
| `grw_repl/src/expr.rs` | Create | `Expr`, `BinOp`, `Closure` AST types |
| `grw_repl/src/interp.rs` | Create | AST interpreter: `eval()`, `Value` enum |
| `grw_repl/src/error.rs` | Create | `PredError` type for parse/validation errors |

### Tests

| File | Action | Responsibility |
|------|--------|---------------|
| `grw_derive/tests/basic.rs` | Create | Derive macro expansion tests |
| `grw_repl/tests/parse.rs` | Create | Closure parsing tests |
| `grw_repl/tests/interp.rs` | Create | Interpreter evaluation tests |
| `grw_repl/tests/predicate.rs` | Create | End-to-end `compile_predicate` tests |

---

## Task 1: Convert to Cargo workspace

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Create workspace Cargo.toml**

Wrap the existing `[package]` in a workspace. Add a root `[workspace]` section listing all members:

```toml
[workspace]
members = [".", "grw_derive", "grw_repl"]
resolver = "3"
```

Add this block at the top of the existing `Cargo.toml`, before `[package]`.

- [ ] **Step 2: Verify existing crate still compiles**

Run: `cargo check`
Expected: compiles with no errors (workspace with single member so far)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "convert to cargo workspace"
```

---

## Task 2: Create `grw::layout` module with `Val` trait

**Files:**
- Create: `src/layout/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/layout/mod.rs` with types and trait**

```rust
#[derive(Debug, Clone, Copy)]
pub enum FieldType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    String,
    Struct(fn() -> &'static [FieldInfo]),
}

impl PartialEq for FieldType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool, Self::Bool)
            | (Self::I8, Self::I8)
            | (Self::I16, Self::I16)
            | (Self::I32, Self::I32)
            | (Self::I64, Self::I64)
            | (Self::U8, Self::U8)
            | (Self::U16, Self::U16)
            | (Self::U32, Self::U32)
            | (Self::U64, Self::U64)
            | (Self::F32, Self::F32)
            | (Self::F64, Self::F64)
            | (Self::String, Self::String) => true,
            (Self::Struct(a), Self::Struct(b)) => std::ptr::fn_addr_eq(*a, *b),
            _ => false,
        }
    }
}

impl Eq for FieldType {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldInfo {
    pub name: &'static str,
    pub ty: FieldType,
    pub offset: usize,
}

pub trait Val: 'static {
    fn fields() -> &'static [FieldInfo];
    fn size() -> usize;
    fn align() -> usize;
}
```

- [ ] **Step 2: Add module to `src/lib.rs`**

Add after the existing `pub mod grb;` line:

```rust
pub mod layout;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add src/layout/mod.rs src/lib.rs
git commit -m "add grw::layout module with Val trait"
```

---

## Task 3: Create `grw_derive` proc-macro crate

**Files:**
- Create: `grw_derive/Cargo.toml`
- Create: `grw_derive/src/lib.rs`
- Create: `grw_derive/src/val.rs`

- [ ] **Step 1: Create `grw_derive/Cargo.toml`**

```toml
[package]
name = "grw_derive"
version = "0.1.0"
edition = "2024"

[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full"] }
quote = "1"
proc-macro2 = "1"
```

- [ ] **Step 2: Create `grw_derive/src/lib.rs`**

```rust
mod val;

use proc_macro::TokenStream;

#[proc_macro_derive(Val)]
pub fn derive_val(input: TokenStream) -> TokenStream {
    val::expand(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
```

- [ ] **Step 3: Create `grw_derive/src/val.rs`**

The derive expansion must:
1. Parse the input struct (reject enums/unions)
2. For each named field, determine its `FieldType` from the Rust type
3. Generate `offset_of!(StructName, field_name)` for each field
4. Handle nested structs: if a field's type is not a primitive, assume it implements `Val` and use `FieldType::Struct(<Type as Val>::fields)` (function pointer, no call)
5. Emit `impl grw::layout::Val for StructName { ... }`

```rust
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, PathSegment};

pub fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = syn::parse2(input)?;
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => return Err(syn::Error::new_spanned(&input, "Val requires named fields")),
        },
        _ => return Err(syn::Error::new_spanned(&input, "Val can only be derived for structs")),
    };

    let field_entries: Vec<TokenStream> = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let ty = &f.ty;
        let field_type_expr = type_to_field_type(ty);

        quote! {
            grw::layout::FieldInfo {
                name: #field_name_str,
                ty: #field_type_expr,
                offset: std::mem::offset_of!(#name, #field_name),
            }
        }
    }).collect();

    let field_count = field_entries.len();

    Ok(quote! {
        impl grw::layout::Val for #name {
            fn fields() -> &'static [grw::layout::FieldInfo] {
                static FIELDS: std::sync::LazyLock<[grw::layout::FieldInfo; #field_count]> =
                    std::sync::LazyLock::new(|| [
                        #(#field_entries),*
                    ]);
                &*FIELDS
            }

            fn size() -> usize {
                std::mem::size_of::<#name>()
            }

            fn align() -> usize {
                std::mem::align_of::<#name>()
            }
        }
    })
}

fn type_to_field_type(ty: &Type) -> TokenStream {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return primitive_field_type(seg)
                .unwrap_or_else(|| {
                    let ty = &tp.path;
                    quote! { grw::layout::FieldType::Struct(<#ty as grw::layout::Val>::fields) }
                });
        }
    }
    quote! { compile_error!("unsupported field type for Val derive") }
}

fn primitive_field_type(seg: &PathSegment) -> Option<TokenStream> {
    let ft = match seg.ident.to_string().as_str() {
        "bool" => quote! { grw::layout::FieldType::Bool },
        "i8"   => quote! { grw::layout::FieldType::I8 },
        "i16"  => quote! { grw::layout::FieldType::I16 },
        "i32"  => quote! { grw::layout::FieldType::I32 },
        "i64"  => quote! { grw::layout::FieldType::I64 },
        "u8"   => quote! { grw::layout::FieldType::U8 },
        "u16"  => quote! { grw::layout::FieldType::U16 },
        "u32"  => quote! { grw::layout::FieldType::U32 },
        "u64"  => quote! { grw::layout::FieldType::U64 },
        "f32"  => quote! { grw::layout::FieldType::F32 },
        "f64"  => quote! { grw::layout::FieldType::F64 },
        "String" => quote! { grw::layout::FieldType::String },
        _ => return None,
    };
    Some(ft)
}
```

- [ ] **Step 4: Add `grw_derive` as a dependency of `grw`**

In `grw/Cargo.toml` under `[dependencies]`, add:

```toml
grw_derive = { path = "grw_derive" }
```

- [ ] **Step 5: Re-export the derive macro from `grw`**

In `src/layout/mod.rs`, add at the top:

```rust
pub use grw_derive::Val;
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 7: Commit**

```bash
git add grw_derive/ Cargo.toml src/layout/mod.rs
git commit -m "add grw_derive crate with #[derive(Val)]"
```

---

## Task 4: Test `#[derive(Val)]`

**Files:**
- Create: `grw_derive/tests/basic.rs`

- [ ] **Step 1: Write derive tests**

```rust
use grw::layout::{Val, FieldType};

#[derive(Val)]
struct Simple {
    x: f64,
    y: f64,
    active: bool,
}

#[derive(Val)]
struct Nested {
    label: String,
    coords: Simple,
}

#[test]
fn simple_fields() {
    let fields = Simple::fields();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].name, "x");
    assert_eq!(fields[0].ty, FieldType::F64);
    assert_eq!(fields[0].offset, std::mem::offset_of!(Simple, x));
    assert_eq!(fields[1].name, "y");
    assert_eq!(fields[1].ty, FieldType::F64);
    assert_eq!(fields[2].name, "active");
    assert_eq!(fields[2].ty, FieldType::Bool);
}

#[test]
fn simple_size_align() {
    assert_eq!(Simple::size(), std::mem::size_of::<Simple>());
    assert_eq!(Simple::align(), std::mem::align_of::<Simple>());
}

#[test]
fn nested_struct_field() {
    let fields = Nested::fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "label");
    assert_eq!(fields[0].ty, FieldType::String);
    assert_eq!(fields[1].name, "coords");
    match fields[1].ty {
        FieldType::Struct(inner_fn) => {
            let inner = inner_fn();
            assert_eq!(inner.len(), 3);
            assert_eq!(inner[0].name, "x");
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn all_primitive_types() {
    #[derive(Val)]
    struct AllPrims {
        a: bool,
        b: i8,
        c: i16,
        d: i32,
        e: i64,
        f: u8,
        g: u16,
        h: u32,
        i: u64,
        j: f32,
        k: f64,
        l: String,
    }
    let fields = AllPrims::fields();
    assert_eq!(fields.len(), 12);
    assert_eq!(fields[0].ty, FieldType::Bool);
    assert_eq!(fields[1].ty, FieldType::I8);
    assert_eq!(fields[2].ty, FieldType::I16);
    assert_eq!(fields[3].ty, FieldType::I32);
    assert_eq!(fields[4].ty, FieldType::I64);
    assert_eq!(fields[5].ty, FieldType::U8);
    assert_eq!(fields[6].ty, FieldType::U16);
    assert_eq!(fields[7].ty, FieldType::U32);
    assert_eq!(fields[8].ty, FieldType::U64);
    assert_eq!(fields[9].ty, FieldType::F32);
    assert_eq!(fields[10].ty, FieldType::F64);
    assert_eq!(fields[11].ty, FieldType::String);
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p grw_derive`
Expected: all 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add grw_derive/tests/basic.rs
git commit -m "add grw_derive tests"
```

---

## Task 5: Create `grw_repl` crate with expression AST

**Files:**
- Create: `grw_repl/Cargo.toml`
- Create: `grw_repl/src/lib.rs`
- Create: `grw_repl/src/expr.rs`
- Create: `grw_repl/src/error.rs`

- [ ] **Step 1: Create `grw_repl/Cargo.toml`**

```toml
[package]
name = "grw_repl"
version = "0.1.0"
edition = "2024"

[dependencies]
grw = { path = ".." }
syn = { version = "2", features = ["full"] }
```

- [ ] **Step 2: Create `grw_repl/src/expr.rs`**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Closure {
    pub param: String,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Field { base: String, path: Vec<String> },
    LitInt(i64),
    LitFloat(f64),
    LitBool(bool),
    LitStr(String),
    BinOp { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    UnaryNot(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}
```

- [ ] **Step 3: Create `grw_repl/src/error.rs`**

```rust
#[derive(Debug)]
pub enum PredError {
    Syntax(String),
    UnsupportedExpr(String),
    UnknownField { field: String, available: Vec<String> },
    NestedAccessOnPrimitive { field: String, ty: String },
    NotBoolReturn,
}

impl std::fmt::Display for PredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PredError::Syntax(msg) => write!(f, "syntax error: {msg}"),
            PredError::UnsupportedExpr(msg) => write!(f, "unsupported expression: {msg}"),
            PredError::UnknownField { field, available } => {
                write!(f, "unknown field `{field}`, available: {}", available.join(", "))
            }
            PredError::NestedAccessOnPrimitive { field, ty } => {
                write!(f, "`{field}` is {ty}, cannot access fields on it")
            }
            PredError::NotBoolReturn => write!(f, "predicate must return bool"),
        }
    }
}

impl std::error::Error for PredError {}
```

- [ ] **Step 4: Create `grw_repl/src/lib.rs`**

```rust
pub mod expr;
pub mod error;
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p grw_repl`
Expected: compiles with no errors

- [ ] **Step 6: Commit**

```bash
git add grw_repl/
git commit -m "add grw_repl crate with Expr AST and error types"
```

---

## Task 6: Closure parser (`grw_repl::parse`)

**Files:**
- Create: `grw_repl/src/parse.rs`
- Modify: `grw_repl/src/lib.rs`
- Create: `grw_repl/tests/parse.rs`

- [ ] **Step 1: Write failing parser tests**

Create `grw_repl/tests/parse.rs`:

```rust
use grw_repl::expr::{Expr, BinOp, Closure};
use grw_repl::parse::parse_closure;

#[test]
fn simple_field_gt_literal() {
    let c = parse_closure("|nv| nv.weight > 100.0").unwrap();
    assert_eq!(c.param, "nv");
    assert_eq!(c.body, Expr::BinOp {
        op: BinOp::Gt,
        lhs: Box::new(Expr::Field { base: "nv".into(), path: vec!["weight".into()] }),
        rhs: Box::new(Expr::LitFloat(100.0)),
    });
}

#[test]
fn boolean_and() {
    let c = parse_closure("|nv| nv.x > 1 && nv.y < 2").unwrap();
    assert_eq!(c.param, "nv");
    match &c.body {
        Expr::BinOp { op: BinOp::And, .. } => {}
        other => panic!("expected And, got {other:?}"),
    }
}

#[test]
fn unary_not() {
    let c = parse_closure("|nv| !nv.active").unwrap();
    assert_eq!(c.param, "nv");
    match &c.body {
        Expr::UnaryNot(inner) => match inner.as_ref() {
            Expr::Field { base, path } => {
                assert_eq!(base, "nv");
                assert_eq!(path, &["active"]);
            }
            other => panic!("expected Field, got {other:?}"),
        },
        other => panic!("expected UnaryNot, got {other:?}"),
    }
}

#[test]
fn nested_field_access() {
    let c = parse_closure("|nv| nv.pos.x > 0.0").unwrap();
    match &c.body {
        Expr::BinOp { lhs, .. } => match lhs.as_ref() {
            Expr::Field { base, path } => {
                assert_eq!(base, "nv");
                assert_eq!(path, &["pos", "x"]);
            }
            other => panic!("expected nested Field, got {other:?}"),
        },
        other => panic!("expected BinOp, got {other:?}"),
    }
}

#[test]
fn string_literal() {
    let c = parse_closure(r#"|nv| nv.name == "alice""#).unwrap();
    match &c.body {
        Expr::BinOp { op: BinOp::Eq, rhs, .. } => {
            assert_eq!(rhs.as_ref(), &Expr::LitStr("alice".into()));
        }
        other => panic!("expected Eq, got {other:?}"),
    }
}

#[test]
fn bool_literal() {
    let c = parse_closure("|nv| nv.active == true").unwrap();
    match &c.body {
        Expr::BinOp { op: BinOp::Eq, rhs, .. } => {
            assert_eq!(rhs.as_ref(), &Expr::LitBool(true));
        }
        other => panic!("expected Eq, got {other:?}"),
    }
}

#[test]
fn arithmetic() {
    let c = parse_closure("|nv| nv.x + nv.y > 10.0").unwrap();
    match &c.body {
        Expr::BinOp { op: BinOp::Gt, lhs, .. } => match lhs.as_ref() {
            Expr::BinOp { op: BinOp::Add, .. } => {}
            other => panic!("expected Add, got {other:?}"),
        },
        other => panic!("expected Gt, got {other:?}"),
    }
}

#[test]
fn parenthesized() {
    let c = parse_closure("|nv| (nv.x > 1) || (nv.y < 2)").unwrap();
    match &c.body {
        Expr::BinOp { op: BinOp::Or, .. } => {}
        other => panic!("expected Or, got {other:?}"),
    }
}

#[test]
fn reject_if_expr() {
    let result = parse_closure("|nv| if nv.x > 0 { true } else { false }");
    assert!(result.is_err());
}

#[test]
fn reject_match_expr() {
    let result = parse_closure("|nv| match nv.x { 0 => true, _ => false }");
    assert!(result.is_err());
}

#[test]
fn reject_let_binding() {
    let result = parse_closure("|nv| { let x = nv.y; x > 0 }");
    assert!(result.is_err());
}

#[test]
fn reject_method_call() {
    let result = parse_closure("|nv| nv.x.abs() > 0.0");
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p grw_repl --test parse`
Expected: compilation error (module `parse` doesn't exist yet)

- [ ] **Step 3: Implement the parser**

Create `grw_repl/src/parse.rs`:

```rust
use crate::expr::{Expr, BinOp, Closure};
use crate::error::PredError;

pub fn parse_closure(src: &str) -> Result<Closure, PredError> {
    let expr: syn::ExprClosure = syn::parse_str(src)
        .map_err(|e| PredError::Syntax(e.to_string()))?;

    if expr.inputs.len() != 1 {
        return Err(PredError::Syntax("closure must have exactly one parameter".into()));
    }

    let param = match &expr.inputs[0] {
        syn::Pat::Ident(pi) => pi.ident.to_string(),
        _ => return Err(PredError::Syntax("closure parameter must be a simple identifier".into())),
    };

    let body = lower_expr(&expr.body)?;
    Ok(Closure { param, body })
}

fn lower_expr(expr: &syn::Expr) -> Result<Expr, PredError> {
    match expr {
        syn::Expr::Binary(b) => {
            let op = lower_binop(&b.op)?;
            let lhs = lower_expr(&b.left)?;
            let rhs = lower_expr(&b.right)?;
            Ok(Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) })
        }
        syn::Expr::Unary(u) => match u.op {
            syn::UnOp::Not(_) => {
                let inner = lower_expr(&u.expr)?;
                Ok(Expr::UnaryNot(Box::new(inner)))
            }
            _ => Err(PredError::UnsupportedExpr("only `!` unary operator is supported".into())),
        },
        syn::Expr::Field(f) => {
            let (base, mut path) = flatten_field_access(f)?;
            path.reverse();
            Ok(Expr::Field { base, path })
        }
        syn::Expr::Lit(lit) => lower_lit(&lit.lit),
        syn::Expr::Path(p) => {
            if let Some(ident) = p.path.get_ident() {
                let s = ident.to_string();
                match s.as_str() {
                    "true" => return Ok(Expr::LitBool(true)),
                    "false" => return Ok(Expr::LitBool(false)),
                    _ => {}
                }
                Ok(Expr::Field { base: s, path: vec![] })
            } else {
                Err(PredError::UnsupportedExpr("qualified paths not supported".into()))
            }
        }
        syn::Expr::Paren(p) => lower_expr(&p.expr),
        syn::Expr::If(_) => Err(PredError::UnsupportedExpr("if expressions not available in REPL".into())),
        syn::Expr::Match(_) => Err(PredError::UnsupportedExpr("match arms not available in REPL".into())),
        syn::Expr::Block(_) => Err(PredError::UnsupportedExpr("block expressions not available in REPL".into())),
        syn::Expr::MethodCall(_) => Err(PredError::UnsupportedExpr("method calls not available in REPL".into())),
        syn::Expr::Closure(_) => Err(PredError::UnsupportedExpr("nested closures not available in REPL".into())),
        _ => Err(PredError::UnsupportedExpr(format!("unsupported expression kind"))),
    }
}

fn flatten_field_access(f: &syn::ExprField) -> Result<(String, Vec<String>), PredError> {
    let field_name = match &f.member {
        syn::Member::Named(ident) => ident.to_string(),
        syn::Member::Unnamed(idx) => return Err(PredError::UnsupportedExpr(
            format!("tuple field access `.{}` not supported", idx.index),
        )),
    };

    match f.base.as_ref() {
        syn::Expr::Field(inner) => {
            let (base, mut path) = flatten_field_access(inner)?;
            path.push(field_name);
            Ok((base, path))
        }
        syn::Expr::Path(p) => {
            let base = p.path.get_ident()
                .ok_or_else(|| PredError::UnsupportedExpr("qualified paths not supported".into()))?
                .to_string();
            Ok((base, vec![field_name]))
        }
        _ => Err(PredError::UnsupportedExpr("complex base expression in field access".into())),
    }
}

fn lower_binop(op: &syn::BinOp) -> Result<BinOp, PredError> {
    match op {
        syn::BinOp::Add(_) => Ok(BinOp::Add),
        syn::BinOp::Sub(_) => Ok(BinOp::Sub),
        syn::BinOp::Mul(_) => Ok(BinOp::Mul),
        syn::BinOp::Div(_) => Ok(BinOp::Div),
        syn::BinOp::Rem(_) => Ok(BinOp::Rem),
        syn::BinOp::Eq(_) => Ok(BinOp::Eq),
        syn::BinOp::Ne(_) => Ok(BinOp::Ne),
        syn::BinOp::Lt(_) => Ok(BinOp::Lt),
        syn::BinOp::Gt(_) => Ok(BinOp::Gt),
        syn::BinOp::Le(_) => Ok(BinOp::Le),
        syn::BinOp::Ge(_) => Ok(BinOp::Ge),
        syn::BinOp::And(_) => Ok(BinOp::And),
        syn::BinOp::Or(_) => Ok(BinOp::Or),
        _ => Err(PredError::UnsupportedExpr(format!("unsupported binary operator"))),
    }
}

fn lower_lit(lit: &syn::Lit) -> Result<Expr, PredError> {
    match lit {
        syn::Lit::Int(i) => {
            let val: i64 = i.base10_parse()
                .map_err(|e| PredError::Syntax(format!("invalid integer: {e}")))?;
            Ok(Expr::LitInt(val))
        }
        syn::Lit::Float(f) => {
            let val: f64 = f.base10_parse()
                .map_err(|e| PredError::Syntax(format!("invalid float: {e}")))?;
            Ok(Expr::LitFloat(val))
        }
        syn::Lit::Bool(b) => Ok(Expr::LitBool(b.value)),
        syn::Lit::Str(s) => Ok(Expr::LitStr(s.value())),
        _ => Err(PredError::UnsupportedExpr("unsupported literal type".into())),
    }
}
```

- [ ] **Step 4: Add `parse` module to `grw_repl/src/lib.rs`**

```rust
pub mod expr;
pub mod error;
pub mod parse;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p grw_repl --test parse`
Expected: all 12 tests pass

- [ ] **Step 6: Commit**

```bash
git add grw_repl/src/parse.rs grw_repl/src/lib.rs grw_repl/tests/parse.rs
git commit -m "add syn-based closure parser"
```

---

## Task 7: AST interpreter (`grw_repl::interp`)

**Files:**
- Create: `grw_repl/src/interp.rs`
- Modify: `grw_repl/src/lib.rs`
- Create: `grw_repl/tests/interp.rs`

- [ ] **Step 1: Write failing interpreter tests**

Create `grw_repl/tests/interp.rs`:

```rust
use grw::layout::{Val, FieldInfo, FieldType};
use grw_repl::interp::{eval, Value};
use grw_repl::expr::{Expr, BinOp};

#[derive(grw::layout::Val)]
struct Point {
    x: f64,
    y: f64,
    active: bool,
}

fn ptr_of<T>(val: &T) -> *const u8 {
    val as *const T as *const u8
}

#[test]
fn read_f64_field() {
    let p = Point { x: 3.14, y: 2.72, active: true };
    let expr = Expr::Field { base: "nv".into(), path: vec!["x".into()] };
    let result = eval(&expr, Point::fields(), ptr_of(&p));
    assert_eq!(result, Value::Float(3.14));
}

#[test]
fn read_bool_field() {
    let p = Point { x: 0.0, y: 0.0, active: true };
    let expr = Expr::Field { base: "nv".into(), path: vec!["active".into()] };
    let result = eval(&expr, Point::fields(), ptr_of(&p));
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn compare_gt() {
    let p = Point { x: 5.0, y: 0.0, active: false };
    let expr = Expr::BinOp {
        op: BinOp::Gt,
        lhs: Box::new(Expr::Field { base: "nv".into(), path: vec!["x".into()] }),
        rhs: Box::new(Expr::LitFloat(3.0)),
    };
    let result = eval(&expr, Point::fields(), ptr_of(&p));
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn boolean_and_true() {
    let p = Point { x: 5.0, y: 1.0, active: true };
    let expr = Expr::BinOp {
        op: BinOp::And,
        lhs: Box::new(Expr::BinOp {
            op: BinOp::Gt,
            lhs: Box::new(Expr::Field { base: "nv".into(), path: vec!["x".into()] }),
            rhs: Box::new(Expr::LitFloat(3.0)),
        }),
        rhs: Box::new(Expr::Field { base: "nv".into(), path: vec!["active".into()] }),
    };
    let result = eval(&expr, Point::fields(), ptr_of(&p));
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn boolean_and_false() {
    let p = Point { x: 1.0, y: 0.0, active: true };
    let expr = Expr::BinOp {
        op: BinOp::And,
        lhs: Box::new(Expr::BinOp {
            op: BinOp::Gt,
            lhs: Box::new(Expr::Field { base: "nv".into(), path: vec!["x".into()] }),
            rhs: Box::new(Expr::LitFloat(3.0)),
        }),
        rhs: Box::new(Expr::Field { base: "nv".into(), path: vec!["active".into()] }),
    };
    let result = eval(&expr, Point::fields(), ptr_of(&p));
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn unary_not() {
    let p = Point { x: 0.0, y: 0.0, active: false };
    let expr = Expr::UnaryNot(Box::new(Expr::Field {
        base: "nv".into(),
        path: vec!["active".into()],
    }));
    let result = eval(&expr, Point::fields(), ptr_of(&p));
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn arithmetic_add() {
    let p = Point { x: 3.0, y: 4.0, active: false };
    let expr = Expr::BinOp {
        op: BinOp::Add,
        lhs: Box::new(Expr::Field { base: "nv".into(), path: vec!["x".into()] }),
        rhs: Box::new(Expr::Field { base: "nv".into(), path: vec!["y".into()] }),
    };
    let result = eval(&expr, Point::fields(), ptr_of(&p));
    assert_eq!(result, Value::Float(7.0));
}

#[test]
fn int_literal_comparison() {
    let expr = Expr::BinOp {
        op: BinOp::Gt,
        lhs: Box::new(Expr::LitInt(10)),
        rhs: Box::new(Expr::LitInt(5)),
    };
    let result = eval(&expr, &[], std::ptr::null());
    assert_eq!(result, Value::Bool(true));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p grw_repl --test interp`
Expected: compilation error (module `interp` doesn't exist yet)

- [ ] **Step 3: Implement the interpreter**

Create `grw_repl/src/interp.rs`:

```rust
use grw::layout::{FieldInfo, FieldType};
use crate::expr::{Expr, BinOp};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Str(String),
}

pub fn eval(expr: &Expr, layout: &[FieldInfo], ptr: *const u8) -> Value {
    match expr {
        Expr::Field { base: _, path } => read_field(layout, ptr, path),
        Expr::LitInt(v) => Value::Int(*v),
        Expr::LitFloat(v) => Value::Float(*v),
        Expr::LitBool(v) => Value::Bool(*v),
        Expr::LitStr(v) => Value::Str(v.clone()),
        Expr::BinOp { op, lhs, rhs } => {
            let l = eval(lhs, layout, ptr);
            let r = eval(rhs, layout, ptr);
            eval_binop(*op, l, r)
        }
        Expr::UnaryNot(inner) => {
            let v = eval(inner, layout, ptr);
            match v {
                Value::Bool(b) => Value::Bool(!b),
                other => panic!("cannot apply `!` to {other:?}"),
            }
        }
    }
}

fn read_field(layout: &[FieldInfo], ptr: *const u8, path: &[String]) -> Value {
    let name = &path[0];
    let field = layout.iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("field `{name}` not found in layout"));

    if path.len() > 1 {
        match &field.ty {
            FieldType::Struct(inner_fn) => {
                let inner_ptr = unsafe { ptr.add(field.offset) };
                read_field(inner_fn(), inner_ptr, &path[1..])
            }
            other => panic!("cannot access nested field on {other:?}"),
        }
    } else {
        read_value(field, ptr)
    }
}

fn read_value(field: &FieldInfo, ptr: *const u8) -> Value {
    let p = unsafe { ptr.add(field.offset) };
    match field.ty {
        FieldType::Bool => Value::Bool(unsafe { *(p as *const bool) }),
        FieldType::I8 => Value::Int(unsafe { *(p as *const i8) } as i64),
        FieldType::I16 => Value::Int(unsafe { *(p as *const i16) } as i64),
        FieldType::I32 => Value::Int(unsafe { *(p as *const i32) } as i64),
        FieldType::I64 => Value::Int(unsafe { *(p as *const i64) }),
        FieldType::U8 => Value::Uint(unsafe { *(p as *const u8) } as u64),
        FieldType::U16 => Value::Uint(unsafe { *(p as *const u16) } as u64),
        FieldType::U32 => Value::Uint(unsafe { *(p as *const u32) } as u64),
        FieldType::U64 => Value::Uint(unsafe { *(p as *const u64) }),
        FieldType::F32 => Value::Float(unsafe { *(p as *const f32) } as f64),
        FieldType::F64 => Value::Float(unsafe { *(p as *const f64) }),
        FieldType::String => {
            let s = unsafe { &*(p as *const String) };
            Value::Str(s.clone())
        }
        FieldType::Struct(_) => panic!("cannot read struct field as scalar value"),
    }
}

fn eval_binop(op: BinOp, l: Value, r: Value) -> Value {
    match op {
        BinOp::And => match (l, r) {
            (Value::Bool(a), Value::Bool(b)) => Value::Bool(a && b),
            (l, r) => panic!("cannot apply `&&` to {l:?} and {r:?}"),
        },
        BinOp::Or => match (l, r) {
            (Value::Bool(a), Value::Bool(b)) => Value::Bool(a || b),
            (l, r) => panic!("cannot apply `||` to {l:?} and {r:?}"),
        },
        _ => eval_binop_numeric(op, l, r),
    }
}

fn eval_binop_numeric(op: BinOp, l: Value, r: Value) -> Value {
    let (l, r) = coerce_numeric(l, r);
    match (op, l, r) {
        (BinOp::Add, Value::Float(a), Value::Float(b)) => Value::Float(a + b),
        (BinOp::Sub, Value::Float(a), Value::Float(b)) => Value::Float(a - b),
        (BinOp::Mul, Value::Float(a), Value::Float(b)) => Value::Float(a * b),
        (BinOp::Div, Value::Float(a), Value::Float(b)) => Value::Float(a / b),
        (BinOp::Rem, Value::Float(a), Value::Float(b)) => Value::Float(a % b),
        (BinOp::Eq, Value::Float(a), Value::Float(b)) => Value::Bool(a == b),
        (BinOp::Ne, Value::Float(a), Value::Float(b)) => Value::Bool(a != b),
        (BinOp::Lt, Value::Float(a), Value::Float(b)) => Value::Bool(a < b),
        (BinOp::Gt, Value::Float(a), Value::Float(b)) => Value::Bool(a > b),
        (BinOp::Le, Value::Float(a), Value::Float(b)) => Value::Bool(a <= b),
        (BinOp::Ge, Value::Float(a), Value::Float(b)) => Value::Bool(a >= b),
        (BinOp::Add, Value::Int(a), Value::Int(b)) => Value::Int(a + b),
        (BinOp::Sub, Value::Int(a), Value::Int(b)) => Value::Int(a - b),
        (BinOp::Mul, Value::Int(a), Value::Int(b)) => Value::Int(a * b),
        (BinOp::Div, Value::Int(a), Value::Int(b)) => Value::Int(a / b),
        (BinOp::Rem, Value::Int(a), Value::Int(b)) => Value::Int(a % b),
        (BinOp::Eq, Value::Int(a), Value::Int(b)) => Value::Bool(a == b),
        (BinOp::Ne, Value::Int(a), Value::Int(b)) => Value::Bool(a != b),
        (BinOp::Lt, Value::Int(a), Value::Int(b)) => Value::Bool(a < b),
        (BinOp::Gt, Value::Int(a), Value::Int(b)) => Value::Bool(a > b),
        (BinOp::Le, Value::Int(a), Value::Int(b)) => Value::Bool(a <= b),
        (BinOp::Ge, Value::Int(a), Value::Int(b)) => Value::Bool(a >= b),
        (op, Value::Str(a), Value::Str(b)) => match op {
            BinOp::Eq => Value::Bool(a == b),
            BinOp::Ne => Value::Bool(a != b),
            _ => panic!("cannot apply {op:?} to strings"),
        },
        (op, l, r) => panic!("cannot apply {op:?} to {l:?} and {r:?}"),
    }
}

fn coerce_numeric(l: Value, r: Value) -> (Value, Value) {
    match (&l, &r) {
        (Value::Float(_), Value::Int(i)) => (l, Value::Float(*i as f64)),
        (Value::Int(i), Value::Float(_)) => (Value::Float(*i as f64), r),
        (Value::Float(_), Value::Uint(u)) => (l, Value::Float(*u as f64)),
        (Value::Uint(u), Value::Float(_)) => (Value::Float(*u as f64), r),
        (Value::Int(i), Value::Uint(u)) => (Value::Int(*i), Value::Int(*u as i64)),
        (Value::Uint(u), Value::Int(i)) => (Value::Int(*u as i64), Value::Int(*i)),
        (Value::Uint(a), Value::Uint(b)) => (Value::Int(*a as i64), Value::Int(*b as i64)),
        _ => (l, r),
    }
}
```

- [ ] **Step 4: Add `interp` module to `grw_repl/src/lib.rs`**

```rust
pub mod expr;
pub mod error;
pub mod parse;
pub mod interp;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p grw_repl --test interp`
Expected: all 8 tests pass

- [ ] **Step 6: Commit**

```bash
git add grw_repl/src/interp.rs grw_repl/src/lib.rs grw_repl/tests/interp.rs
git commit -m "add AST interpreter for value predicates"
```

---

## Task 8: `compile_predicate` public API with validation

**Files:**
- Modify: `grw_repl/src/lib.rs`
- Create: `grw_repl/tests/predicate.rs`

- [ ] **Step 1: Write failing end-to-end tests**

Create `grw_repl/tests/predicate.rs`:

```rust
use grw::layout::Val;
use grw_repl::compile_predicate;

#[derive(Val)]
struct Cargo {
    weight: f64,
    fragile: bool,
    category: u32,
}

#[derive(Val)]
struct Located {
    name: String,
    pos: Coords,
}

#[derive(Val)]
struct Coords {
    x: f64,
    y: f64,
}

#[test]
fn weight_gt() {
    let pred = compile_predicate::<Cargo>("|c| c.weight > 50.0").unwrap();
    assert!(pred(&Cargo { weight: 100.0, fragile: false, category: 1 }));
    assert!(!pred(&Cargo { weight: 10.0, fragile: false, category: 1 }));
}

#[test]
fn bool_field() {
    let pred = compile_predicate::<Cargo>("|c| c.fragile").unwrap();
    assert!(pred(&Cargo { weight: 0.0, fragile: true, category: 0 }));
    assert!(!pred(&Cargo { weight: 0.0, fragile: false, category: 0 }));
}

#[test]
fn negated_bool() {
    let pred = compile_predicate::<Cargo>("|c| !c.fragile").unwrap();
    assert!(!pred(&Cargo { weight: 0.0, fragile: true, category: 0 }));
    assert!(pred(&Cargo { weight: 0.0, fragile: false, category: 0 }));
}

#[test]
fn compound_and() {
    let pred = compile_predicate::<Cargo>("|c| c.weight > 50.0 && !c.fragile").unwrap();
    assert!(pred(&Cargo { weight: 100.0, fragile: false, category: 0 }));
    assert!(!pred(&Cargo { weight: 100.0, fragile: true, category: 0 }));
    assert!(!pred(&Cargo { weight: 10.0, fragile: false, category: 0 }));
}

#[test]
fn uint_eq() {
    let pred = compile_predicate::<Cargo>("|c| c.category == 3").unwrap();
    assert!(pred(&Cargo { weight: 0.0, fragile: false, category: 3 }));
    assert!(!pred(&Cargo { weight: 0.0, fragile: false, category: 1 }));
}

#[test]
fn nested_field() {
    let pred = compile_predicate::<Located>("|l| l.pos.x > 0.0").unwrap();
    assert!(pred(&Located { name: String::from("a"), pos: Coords { x: 1.0, y: 0.0 } }));
    assert!(!pred(&Located { name: String::from("b"), pos: Coords { x: -1.0, y: 0.0 } }));
}

#[test]
fn string_eq() {
    let pred = compile_predicate::<Located>(r#"|l| l.name == "alice""#).unwrap();
    assert!(pred(&Located { name: String::from("alice"), pos: Coords { x: 0.0, y: 0.0 } }));
    assert!(!pred(&Located { name: String::from("bob"), pos: Coords { x: 0.0, y: 0.0 } }));
}

#[test]
fn unknown_field_error() {
    let err = compile_predicate::<Cargo>("|c| c.missing > 0").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("missing"), "error should mention the field name: {msg}");
}

#[test]
fn nested_on_primitive_error() {
    let err = compile_predicate::<Cargo>("|c| c.weight.x > 0.0").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("weight"), "error should mention the field: {msg}");
}

#[test]
fn non_bool_return_error() {
    let err = compile_predicate::<Cargo>("|c| c.weight + 1.0").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("bool"), "error should mention bool return: {msg}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p grw_repl --test predicate`
Expected: compilation error (`compile_predicate` doesn't exist yet)

- [ ] **Step 3: Implement `compile_predicate` with validation**

Update `grw_repl/src/lib.rs`:

```rust
pub mod expr;
pub mod error;
pub mod parse;
pub mod interp;

use grw::layout::{Val, FieldInfo, FieldType};
use error::PredError;
use expr::Expr;

pub fn compile_predicate<NV: Val>(
    src: &str,
) -> Result<Box<dyn Fn(&NV) -> bool>, PredError> {
    let closure = parse::parse_closure(src)?;
    let layout = NV::fields();
    validate_expr(&closure.body, layout)?;
    if !returns_bool(&closure.body, layout) {
        return Err(PredError::NotBoolReturn);
    }
    let fields = layout;
    Ok(Box::new(move |nv: &NV| {
        let ptr = nv as *const NV as *const u8;
        match interp::eval(&closure.body, fields, ptr) {
            interp::Value::Bool(b) => b,
            other => panic!("predicate must return bool, got {other:?}"),
        }
    }))
}

fn validate_expr(expr: &Expr, layout: &[FieldInfo]) -> Result<(), PredError> {
    match expr {
        Expr::Field { base: _, path } => {
            if path.is_empty() {
                return Err(PredError::UnsupportedExpr(
                    "bare parameter reference without field access".into(),
                ));
            }
            validate_field_path(layout, path)
        }
        Expr::LitInt(_) | Expr::LitFloat(_) | Expr::LitBool(_) | Expr::LitStr(_) => Ok(()),
        Expr::BinOp { lhs, rhs, .. } => {
            validate_expr(lhs, layout)?;
            validate_expr(rhs, layout)
        }
        Expr::UnaryNot(inner) => validate_expr(inner, layout),
    }
}

fn returns_bool(expr: &Expr, layout: &[FieldInfo]) -> bool {
    match expr {
        Expr::LitBool(_) => true,
        Expr::UnaryNot(_) => true,
        Expr::BinOp { op, .. } => matches!(
            op,
            expr::BinOp::Eq
                | expr::BinOp::Ne
                | expr::BinOp::Lt
                | expr::BinOp::Gt
                | expr::BinOp::Le
                | expr::BinOp::Ge
                | expr::BinOp::And
                | expr::BinOp::Or
        ),
        Expr::Field { path, .. } => {
            if path.is_empty() {
                return false;
            }
            match resolve_field_type(layout, path) {
                Some(FieldType::Bool) => true,
                _ => false,
            }
        }
        _ => false,
    }
}

fn resolve_field_type(layout: &[FieldInfo], path: &[String]) -> Option<FieldType> {
    if path.is_empty() {
        return None;
    }
    let field = layout.iter().find(|f| f.name == path[0])?;
    if path.len() == 1 {
        Some(field.ty)
    } else {
        match &field.ty {
            FieldType::Struct(inner_fn) => resolve_field_type(inner_fn(), &path[1..]),
            _ => None,
        }
    }
}

fn validate_field_path(layout: &[FieldInfo], path: &[String]) -> Result<(), PredError> {
    let name = &path[0];
    let field = layout.iter()
        .find(|f| f.name == name)
        .ok_or_else(|| PredError::UnknownField {
            field: name.clone(),
            available: layout.iter().map(|f| f.name.to_string()).collect(),
        })?;

    if path.len() > 1 {
        match &field.ty {
            FieldType::Struct(inner_fn) => validate_field_path(inner_fn(), &path[1..]),
            other => Err(PredError::NestedAccessOnPrimitive {
                field: name.clone(),
                ty: format!("{other:?}"),
            }),
        }
    } else {
        Ok(())
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p grw_repl --test predicate`
Expected: all 10 tests pass

- [ ] **Step 5: Run all tests across the workspace**

Run: `cargo test --workspace`
Expected: all tests pass (grw existing tests + grw_derive tests + grw_repl tests)

- [ ] **Step 6: Commit**

```bash
git add grw_repl/src/lib.rs grw_repl/tests/predicate.rs
git commit -m "add compile_predicate API with field validation"
```
