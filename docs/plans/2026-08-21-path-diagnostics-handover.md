# Handover — path/search DSL compile-error diagnostics

**Date:** 2026-08-21
**Repo:** `~/Dev/grw` (branch `master`, HEAD `5c0600d` + uncommitted changes listed below)
**Working doc — do NOT include in the public curated history.**

---

## 1. Where we are

Goal of the whole effort: give the DSLs **custom compile errors** (via
`#[diagnostic::on_unimplemented]`) for invalid constructions, matching the
quality already present in `graph!`/`modify!` (see `src/graph/edge.rs`
`Src`/`Tgt`/`Und` traits: `message` + `label` + `note`, domain language).

### Done and verified this session
- **README acknowledgement reworded** (theory + original impl = ~2y manual
  work; Claude took over implementation from 2026).
- **Path DSL diagnostics that WORK** (both render, both locked by trybuild):
  - `Len` (bad `.len(..)`, e.g. `&str`) → "not a valid path length…"
  - `Navigator` (bad `.navigate(..)`, e.g. integer) → "not a path navigator…"
  - Fixtures: `tests/compile_fail/path_len_not_a_range.{rs,stderr}`,
    `path_navigate_not_a_navigator.{rs,stderr}`.
  - Docs added on `Len` and `Navigator` in `src/search/path.rs`.
- **Search matching**: edge-direction misuse is ALREADY covered — the search
  edge ops (`src/search/dsl/edge.rs`) reuse `graph::edge::{Src,Tgt,Und}` bounds,
  so their `on_unimplemented` fires. `validate.rs` checks (duplicate-def,
  undefined-ref, context-in-ban) are legitimately **runtime** (depend on
  cross-cluster structure). So little to add there.

### Uncommitted working-tree state (on `master`, NOT committed — repo rule: user initiates git)
- `README.md` — acknowledgement reworded
- `src/search/path.rs` — `Len` + `Navigator` `on_unimplemented` + docs
- `src/search/dsl/node.rs` — `IntoPathConfig` `on_unimplemented` **(INEFFECTIVE — see §2; harmless, decide whether to keep)**
- `tests/compile_fail/path_len_not_a_range.{rs,stderr}` (new)
- `tests/compile_fail/path_navigate_not_a_navigator.{rs,stderr}` (new)
- `LICENSE-APACHE`, `LICENSE-MIT` (untracked; created for the curation step, see §10)

---

## 2. The core finding (drives everything)

`#[diagnostic::on_unimplemented]` fires **only for unsatisfied trait bounds
(E0277)**. It does NOT touch **method-not-found (E0599)**.

- `Len`/`Navigator` work because they are bounds on method *arguments*
  (`bounds: impl Len`, `Nav: Navigator<EV>`) — a bad arg is an E0277.
- The remaining path mistakes are **E0599** and thus uncustomizable as-is:
  - `.one()` / `.all()` before `.navigate()` (they live only on `Config<_, Navigated, _>`)
  - `.dfs().navigate()` / any double-mode (`dfs`/`bfs`/`navigate` live only on `Config<_, Unset, _>`)
  - bad path source `..<non-node>` (`IntoPathConfig` not implemented) — method resolution, so the `IntoPathConfig` `on_unimplemented` I added does NOT fire. (That is why it is marked ineffective above.)

**Verified empirically** (probe: `/tmp/.../scratchpad/diag_probe.rs`):
- stable **1.96.0** and nightly **1.98.0-nightly**: the public `diagnostic::`
  namespace is exactly `on_unimplemented` + `do_not_recommend`. No
  method-resolution diagnostic (`on_unresolved_method` → "unknown diagnostic
  attribute"). `on_unimplemented(on(...))` conditional form → "malformed" (not
  stable; the conditional form only exists as compiler-internal
  `#[rustc_on_unimplemented]` behind `feature(rustc_attrs)`, unusable in real crates).
- Not probed (not installed): stable 1.98.0, nightly 1.100.0 — no reason to
  expect a change.

**Conclusion:** the only way to get a custom message for the E0599 cases is to
**convert them into E0277** — define the method on all `Mode`s with a
`where Mode: Marker` bound and put `on_unimplemented` on `Marker`. This is the
sanctioned mechanism, not a hack.

---

## 3. The task — marker-bound refactor

Add sealed marker traits with diagnostics and re-home the type-state methods so
wrong-state calls become customizable E0277s.

```rust
mod mode_seal { pub trait Sealed {} }

#[diagnostic::on_unimplemented(
    message = "`.one()` and `.all()` apply only to a navigated path",
    label = "not a navigated path",
    note = "call `.navigate(Dijkstra::.. / AStar::..)` first — traversal (`.dfs()`/`.bfs()`) paths have no cost ordering"
)]
pub trait IsNavigated: mode_seal::Sealed {}
impl mode_seal::Sealed for Navigated {}
impl IsNavigated for Navigated {}

#[diagnostic::on_unimplemented(
    message = "a path mode was already chosen — `.dfs()`, `.bfs()`, `.drive()`, `.navigate()` are mutually exclusive",
    label = "path mode already set",
    note = "choose exactly one traversal on a fresh `..node` path"
)]
pub trait IsUnset: mode_seal::Sealed {}
impl mode_seal::Sealed for Unset {}
impl IsUnset for Unset {}
```

Then, in `src/search/path.rs`:
- `one`/`all`: move from `impl<N, EV> Config<N, Navigated, EV>` to
  `impl<N, Mode, EV> Config<N, Mode, EV> where Mode: IsNavigated`. **Bodies
  unchanged** (they already `if let Driver::Navigate(_, mode) = &mut self.driver`).
- `dfs`/`bfs`/`drive`/`navigate`: move from `impl<N, EV> Config<N, Unset, EV>`
  (and the `Config<N, Unset, ()>` block for `navigate`) to
  `impl<N, Mode, EV> Config<N, Mode, EV> where Mode: IsUnset`. Bodies unchanged.

---

## 4. Critical architecture context (read before touching anything)

**Two layers** — do not conflate them:
1. `IntoPathConfig` (`src/search/dsl/node.rs`) has its OWN
   `len/guard/dfs/bfs/drive/navigate` methods **on node refs**
   (Free/FreeRef/Context/ContextRef). `..n(1).dfs()` calls
   `IntoPathConfig::dfs`, which does `Config::new(self).dfs()`. So the *node*
   entry point is here.
2. `Config<N, Mode, EV>` (`src/search/path.rs`) has the type-state methods.
   `config.dfs().navigate()` (double-mode) is a `Config`-level chain.

The marker-bound refactor is at the **`Config`** layer. But note: because
`IntoPathConfig::dfs` returns `Config<_, Traversal, _>` directly, the common
double-mode mistake surfaces as `IntoPathConfig::dfs(...)` → `Config<Traversal>`
→ `.navigate()` (Config-level, Unset-only) → E0599. Fixing `Config::navigate`
to `where Mode: IsUnset` turns THAT into the nice E0277. Good.

**The EV-inference tuning (the fragile part).** `path.rs` has explicit comments:
- "Builders are generic over `EV` so that `.dfs()/.bfs()` configs unify their
  `EV` with the edge type at the path operator — letting a single operator arm
  accept both traversal and navigated paths."
- "`navigate` lives on the `()` config so its receiver `EV` is pinned (the
  navigator brings its own `EV`), avoiding inference ambiguity in chains."

This means `navigate` is currently on `Config<N, Unset, ()>` **specifically to
pin EV = ()**. If you generalise it to `Config<N, Mode, EV> where Mode: IsUnset`
you MUST preserve the EV-pinning, or `..n(1).navigate(Dijkstra::counted())` may
lose inference. Options: keep `navigate` on the `()` receiver but add the
`IsUnset` bound (`impl<N, Mode> Config<N, Mode, ()> where Mode: IsUnset`), or
otherwise ensure EV stays pinned. **Treat `navigate` as the highest-risk method.**

---

## 5. Intricacies & risks — the UX must NOT regress

The hard requirement: **users writing grw path expressions must not suddenly
need type annotations** they didn't need before. That is the failure mode of
this refactor.

Why the risk exists: moving a method from a concrete `impl Config<N, Unset, EV>`
to a generic `impl<Mode> Config<N, Mode, EV> where Mode: IsUnset` changes
inference. When the receiver's `Mode` is not yet pinned at the call site, the
`where Mode: IsUnset` bound can force the solver to *prove* a bound before it
has resolved `Mode`, producing "type annotations needed". Mitigations:

- **Seal the markers** (`mode_seal::Sealed` supertrait, impls only for the three
  mode structs). A sealed marker with exactly one implementor helps inference
  pick the `Mode` unambiguously and prevents external impls.
- Prefer keeping the return types **concrete** (dfs → `Config<_, Traversal, _>`)
  as they are now; only the *receiver* gains the bound.
- Preserve EV-pinning on `navigate` (see §4).
- If a specific method regresses inference even after sealing, **retract just
  that method** (leave it as a concrete-state impl with the default E0599) — a
  nice error is not worth a UX regression. Partial coverage is acceptable; a
  worse authoring experience is not.

---

## 6. UX-preservation oracle (build this FIRST, before changing code)

1. **Baseline:** `cargo test --no-run` (compiles lib + all tests + all
   trybuild-referenced code). Must be clean at baseline. The existing ~95 path
   tests double as positive-usage coverage — if any real usage suddenly needs an
   annotation, they fail to compile.
2. **Dedicated inference-preservation test** — add
   `tests/path_infer_noannot.rs` containing the most inference-sensitive path
   expressions written with **zero type annotations**, e.g.:
   ```rust
   // must all compile annotation-free
   let _ = grw::search::path::Config::new(()).dfs();
   let _ = grw::search::path::Config::new(()).bfs().len(2..5);
   let _ = grw::search::path::Config::new(()).navigate(grw::search::path::Dijkstra::counted());
   let _ = grw::search::path::Config::new(()).navigate(
       grw::search::path::AStar::new(|_ev: &()| 1.0, |_n| 0.0)).all();
   // plus in-search! forms:  get(Mono) { n(0) ^ ..n(1).dfs().len(2..) }, etc.
   ```
   Copy the exact in-`search!` forms from `tests/path_coverage.rs` (e.g. lines
   ~168, ~192) and standalone forms from `tests/path_iter.rs` (~91, 136, 163).
   This file is the **specific guard for the user's concern**: it must keep
   compiling annotation-free after every change.

---

## 7. Lock-step, retractable procedure (per the user's ask)

Do ONE marker at a time; each is a discrete revertable unit.

For each of `IsNavigated` (do first — lowest risk, terminal state), then
`IsUnset` (higher risk, initial state + `navigate` EV-pinning):

1. `git stash` or note current file hashes so retract = `git checkout -- <file>`.
2. Add the marker trait + impls; re-home the method(s).
3. Verify, in order — ALL must hold to keep the change:
   - `cargo build` (lib) clean.
   - `cargo test --no-run` clean → **no positive usage regressed**.
   - `tests/path_infer_noannot.rs` compiles annotation-free.
   - `cargo test --test path_oracle --test path_iter --test path_coverage`
     PASS → **correctness unchanged** (petgraph-oracle validated).
   - New compile-fail case renders the custom message
     (`TRYBUILD=overwrite cargo test --test compile_fail`, then inspect `.stderr`).
4. If any check fails → **`git checkout -- <the file>`** to retract that marker,
   and either try the narrower variant (e.g. keep `navigate` EV-pinned) or leave
   that method as-is (default E0599). Never ship a variant that regresses §6.

Add compile-fail fixtures once the messages are confirmed:
`path_one_before_navigate.rs`, `path_double_mode.rs` (mirror the existing
`tests/compile_fail/*.rs` structure; harness is `tests/compile_fail.rs`).

---

## 8. Correctness & runtime performance — must NOT be lowered

This is guaranteed **a priori** by construction, and must be **confirmed empirically**:
- The marker traits are **empty** (no methods) → zero runtime representation.
- The `where` bounds are **compile-time only** → no codegen.
- The method **bodies are unchanged** → identical machine code; identical behavior.
- Therefore there is **no runtime cost and no behavioral change** — only which
  `impl` block a method textually lives in, plus a compile-time bound.
- **Confirm:** full path test suite (oracle-validated vs petgraph) must pass
  unchanged (§7 step 3). No need to run benches (structurally impossible to
  regress codegen); if desired, a single spot-check bench is unchanged — but the
  user is battery-conscious, and the a-priori argument is airtight.

---

## 9. Remaining path docs (the earlier "needs doc" ask — low risk, no inference concern)

`src/search/path.rs` still has undocumented public items: `Explore` (+ variants
— read the consumer for `One`/`Len`/`All`/`Steps` semantics before documenting;
don't guess), `Dijkstra`/`AStar` constructors, `NavMode` (partly done),
`Config` type-states + builders (`new`/`dfs`/`bfs`/`drive`/`len`/`navigate`/
`one`/`all`), `PathConstraint` methods. Add `///` (only) matching the module's
terse voice. Test coverage is already comprehensive (~95 tests, petgraph oracle)
— do NOT add tests.

---

## 10. Surrounding pending work (so nothing is lost across the model switch)

The bigger context this sits inside:

- **`bench_all.sh` fix:** add a guard that checks the corpus
  (`data/crosscheck/undir`) and generates it if absent (generator is
  `~/Dev/grw_vf3_tests/gen_data.sh` → outputs to that repo's
  `tests/crosscheck/data/undir`; note the cross-repo path coupling). Currently
  it references data + a `verify_patterns` binary that won't be present.
- **Exclude all four scripts** from the public repo: `build_history.sh`,
  `zip_repo.sh`, `scratch.sh`, `bench_all.sh` (and `CLAUDE.md`).
- **Re-curate git history (option C, user-approved).** Backups already exist and
  are verified: `~/backups/grw-20260821-142011/` (local bundle + worktree tar +
  the remote `fdcc469` March-snapshot bundle). Both remotes (`github`,
  `codeberg` = `grw-rs/grw`) currently hold an unrelated March snapshot
  `fdcc469` that LACKS all path work. Plan: build an orphan `curated` branch of
  ~8 logical commits (core → graph! DSL → modify! → search engine → path search
  → persistence → docs/viz/tooling → license), tree byte-identical to `master`
  except the four scripts + `CLAUDE.md` removed and `LICENSE-{APACHE,MIT}` added,
  then `git push --force-with-lease curated:master` to BOTH remotes. The big
  `.grw` data (600MB+) is only in old history, already absent from the current
  tree — the orphan drops it automatically (public tree ≈ 3.6M). A prior build
  of this exact curation succeeded and verified byte-identical; the build script
  is straightforward to reproduce (see session transcript / `build_history.sh`
  as a reference for structure, though that script itself is excluded).
- After the code work lands, **rebuild the curated history including it**, then push.
- **CV context:** grw.rs now hosts a real landing page (valid HTTPS) that the CV
  links to; the CV work (`~/Dev/job/cv/ats/`) is otherwise complete except the
  Independent section's trading-system metrics, which need the user's
  confirmation (sub-100ms latency; 407h replay parity).

## 11. Key references
- Diagnostic pattern to mirror: `src/graph/edge.rs` (`Src`/`Tgt`/`Und`).
- Trybuild harness: `tests/compile_fail.rs`; example pair: `dir_bitxor.{rs,stderr}`.
- Path type-state impls to re-home: `src/search/path.rs` (`one`/`all` on
  `Config<_, Navigated, _>`; `dfs`/`bfs`/`drive` on `Config<_, Unset, EV>`;
  `navigate` on `Config<_, Unset, ()>`).
- Node-layer path entry: `src/search/dsl/node.rs` `IntoPathConfig`.
- Repo rules (`CLAUDE.md`): use `cargo check`; explain intent before changes;
  no co-authored-by / one-line commits; **user initiates git commits**; typestate
  everywhere; no defaults/fallbacks.
