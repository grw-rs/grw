# Host-language bindings design (Python / TypeScript / Elixir)

Status: design agreed 2026-08-28, nothing implemented yet.
Goal: grw usable from Python first, then Elixir, then TypeScript, with
(a) persisted graphs and patterns shared across hosts,
(b) no host-language code in the search loop (no GIL / NIF timeslice / event-loop blocking),
(c) DSL syntax as close to `graph!` / `modify!` / `search!` as each host allows.

## Bridge technology (no C glue anywhere)

| Host | Crate | Notes |
|---|---|---|
| Python | pyo3 + maturin, abi3 wheels | release GIL around every search/modify/persist call |
| Elixir | rustler | graph = NIF resource; long ops on dirty CPU schedulers; lazy sessions stepped per call (batch of matches) |
| TypeScript | napi-rs (Node), wasm-bindgen for browser demo only | search/modify as AsyncTask -> Promise; .d.ts generated |

## Why generics/typestates/macros don't cross, and what replaces them

- `Graph<NV, E>` is generic; pyclass etc. cannot be. Bindings expose concrete classes
  `Undir`, `Dir`, `Anydir` over one dynamic value type `Val`.
- Typestates stay in Rust. Host facade holds a state enum, raises on misuse; static
  checking via stubs/typespecs/TS interfaces (see Typing).
- Macros: DSL fragments are plain structs (README) -> bindings wrap fragment structs and
  forward operators; `from_fragment()` / `modify()` / `compile()` run at the end.

## `layout::Val` is NOT the dynamic value

`layout::Val` (src/graph/layout.rs) is type-level reflection over Rust types: static
field table with byte offsets, `layout_hash()` written into the persist header
(persist.rs), method table for the repl. It describes existing Rust structs; it cannot
represent host values. Bindings need a runtime enum instead.

## Dynamic value: `dyn::Val`

```
Val = Nil | Bool | I64 | F64 | Str | Atom(String) | List(Vec<Val>) | Tuple(Vec<Val>) | Map(Key -> Val)
Key = Str | Atom | I64          (anything else -> explicit error)
```
- One shared Rust type => same `layout_hash` => a persisted graph round-trips
  Python <-> Elixir <-> TS <-> Rust repl. This is why `Val` is NOT per-binding.
- Conversions are per-binding (`PyAny <-> Val`, `Term <-> Val`, JS value <-> Val).
- Storing native host objects (`Py<PyAny>`) rejected: GIL per compare, no rayon, and
  impossible for Elixir (terms are process-bound).
- Decisions (explicit, no fallbacks):
  - integers: I64; overflow -> error (Python/Elixir bignums, JS: accept number|bigint in,
    always bigint out)
  - Elixir `true/false` -> Bool, `nil` -> Nil, other atoms -> Atom. Python has no atoms:
    `grw.Atom("ok")` — interned, hashable, NOT a str subclass (`Atom("ok") == "ok"` is False),
    so matching semantics are identical in all hosts. Same class in TS (not Symbol).
  - Python None / JS null -> Nil; JS undefined -> error
  - Tuple: Python tuple 1:1; JS branded `Tuple` class (plain array = List)
  - Map: JS `Map` (plain object rejected — string keys only would lose atom keys)
  - non-UTF-8 binaries / charlists -> error; add `Bytes` only if needed

## Predicates: CEL, evaluated in Rust only

- `Pred` = CEL expression. Parse with `cel-parser` (separate crate in cel-rust), but
  evaluate with OUR evaluator directly over `Val` (no `cel-interpreter`: its own Value
  type would need per-candidate conversion; also no atoms/tuples).
- Validated at bind (unknown field, arity, type errors -> loud error once); at runtime
  a type mismatch evaluates to `false`, never coercion.
- Atoms/tuples native: `atom('ok')` in CEL text.
- Persistable: pattern graph + predicates is one text document. No native lambdas at all
  (dropped: bitmap precompute rejected as impractical for large graphs; per-candidate
  host callbacks rejected because of GIL).
- Transport to Rust is CEL text, but users never write strings:

| Host | How the expression becomes CEL |
|---|---|
| Python | operator-overloaded `v` placeholder; `& \| ~` for and/or/not (polars/pandas convention). Secondary: `inspect.getsource`+`ast` transpile (fragile in REPL) |
| TypeScript | real arrow fn; `fn.toString()` -> source is near-valid CEL -> Rust parses |
| Elixir | `search` macro receives quoted AST at compile time -> CEL, compile-time validation |

  Rules: transpile is syntactic, CEL semantics win (documented); untranslatable
  construct -> explicit error; captured host variables interpolated as literals.

- Custom Rust functions (CEL host functions): registry `fn(&[Val]) -> Result<Val>`,
  called as `rust.name(...)` in CEL, validated at bind, direct fn-pointer call in loop.
  Tier A now: compiled-in (user builds own wheel/NIF that registers). Tier B later:
  `abi_stable` dynamic plugin `.so` via `load_plugin`. Tier C parked: wasm via `wasmi`.
  Registry is per-process and explicit; no auto-discovery.

## DSL per host

Python (operators overload with SAME precedence order as Rust: >> << > & > ^, left-assoc):
```python
from grw.dsl import *   # N, N_, n, E, e, X, x, get, ban, v
g = graph[N(0).val("a") & E().val(10) ^ (N(1).val("b") ^ n(0))]
modify(g, [X(0) & e().val(200) ^ X(1), ~X(2)])
for m in search[g, get(Mono)[N(0).test(v.age > 18) ^ N(1)], ban(Mono)[n(0) ^ N(2)]]: ...
with g.modify() as m: ...
```
Forced deviations only: `!` -> `~`; `graph![..]` -> `graph[..]` (tuple via __class_getitem__);
`get(M){..}` -> `get(M)[..]`.

TypeScript: no operator overloading -> tagged template literal over the text DSL parser
(`graph\`N(0) ^ (N(1) ^ n(0))\``, values via `${}`), fluent methods as underlay.
Elixir: `~G"..."` sigil over the same parser, or operator macros.

=> a text DSL parser (`dyn::parse`) lives in core-adjacent code; printer already exists
(`graph/dsl_fmt.rs::to_dsl`). Python doesn't strictly need it; TS/Elixir/repl do.

## Typing

- Python: `.pyi` stubs (hand or pyo3-stub-gen): `class Undir(Generic[NV, EV])`, typestates
  as separate stub classes (`bind()` returns ResolvedSearch; `iter()` only there),
  `Morphism` IntEnum, `@grw.val @dataclass` -> Val::Map, `@overload` for edge values.
  Static overlay only — runtime still uses Val.
- TS: real generics + state-parameterised interfaces; napi generates .d.ts.
- Elixir: typespecs.

## Crate layout (decided)

```
grw/  (root stays the core package AND workspace root — no file moves)
  Cargo.toml   add [workspace] members = ["grw-dyn", "grw-py"]
  grw-dyn/     Val (+serde, +impl layout::Val), Pred (cel-parser + evaluator + registry),
               dyn::parse text DSL, non-generic facade over Graph<Val, edge::*<Val>>
  grw-py/      pyo3 + maturin; conversions, operator forwarding, GIL policy, stubs
  grw-ts/ grw-ex/  later, depend on grw-dyn only
```
Not a `feature = "dyn"` in grw (cel-parser dep + parser shouldn't burden Rust-only
users); not inside grw-py (Val must be one shared type for persistence).
If grw-dyn needs a pub(crate) from core, fix the core API publicly instead.

## Order of work

1. root `[workspace]`; `grw-dyn` with `Val` (+serde, `layout::Val` impl, Key restriction,
   I64 overflow errors) + persist round-trip test through `Graph<Val, Anydir<Val>>`
2. `dyn::Graph` facade over graph! fragments (Undir/Dir/Anydir)
3. `dyn::parse` text DSL (graph / modify / search)
4. `Pred`: cel-parser -> validate -> compile over Val; `rust.` registry (Tier A)
5. `grw-py` scaffold (maturin): Val conversion, Atom class, `graph[...]`, then search
   (sessions, translate, par_iter under allow_threads), then modify (context manager)
6. `.pyi` stubs; Python `v` expression builder emitting CEL
7. grw-ex, grw-ts later

Process: per CLAUDE.md — explain intent + pros/cons before each code change, typestates
everywhere, `cargo check`, no defaults/fallbacks, no comments, user commits.
