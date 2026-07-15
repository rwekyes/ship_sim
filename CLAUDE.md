# ship_sim — Project Briefing

## Big rule to follow
This project is primarily a Rust learning project. As such, Claude will not be 
allowed to make edits to files directly. The intention is to allow Claude Code 
to assist in diagnosing bugs, answering questions, and giving guidance. 
Code examples should be kept to a minimum when not asked directly for an 
implementation.
This document is to be the one exception to the file edit rule.

## What this is

A spaceship simulator for tabletop RPG sessions (The Expanse RPG). The GM runs
a server on a laptop; players connect from their own laptops in the browser and
interact with ship systems by role (nav, engineering, etc.) for immersion.
Primary goal is **learning Rust** — prefer idiomatic solutions and explain
non-obvious choices over maximally clever ones.

Repo: github.com/rwekyes/ship_sim (private). Owner is a CIS student and intern
learning Rust; Java background; comfortable with git basics but still building
the mental model — flag git/cargo footguns proactively.

## Architecture

Single authoritative Rust server holds all simulation state. Browser clients
are thin views connected via WebSocket. Clients send commands; server
validates, updates state, broadcasts. Role-based sessions filter what each
client sees (nav station can't see reactor internals). GM can inject failures
server-side.

Cargo workspace, five crates under `crates/`:

- **sim-core** (lib) — pure simulation: orbits, burns, ships, systems, time.
  NO I/O, no async, no rusqlite, no file paths. Must stay WASM-compatible.
- **sim-protocol** (lib) — shared Command/StateUpdate/Role message enums
  (serde tagged enums). Kept thin so it compiles to wasm32 if the frontend
  goes Leptos/Dioxus. Depends on sim-core types.
- **sim-store** (lib) — all SQLite persistence (rusqlite, bundled feature).
  One .db file per solar system. Depends on sim-core.
- **sim-cli** (bin) — Phase 1 nav CLI wrapping sim-core directly.
- **sim-server** (bin) — axum + WebSocket, serves static frontend. Not yet
  started.

Frontend: undecided between TypeScript+Canvas and full-Rust WASM
(Leptos/Dioxus); leaning WASM for learning value. `frontend/` dir, not a
cargo member unless WASM.

`data/` — seed JSON (committed) and generated .db files (gitignored).

## Domain model decisions (settled — don't relitigate)

- **Store Keplerian orbital elements, not position tables.** Position at time
  t computed on demand: mean anomaly → Kepler's equation (Newton iteration) →
  eccentric anomaly → true anomaly → position. Coasting bodies are free.
- **Numerical integration only for ships under thrust.** Semi-implicit Euler
  first, RK4 later. Burns subdivide internally (~60s substeps) within the
  12h/24h game timesteps. After a burn, convert state vector back to elements.
- **Expanse travel = brachistochrone burns** (accelerate, flip, decelerate).
  At sustained ~0.3g, solar gravity is negligible for powered legs — the burn
  solver can be pure kinematics; orbital mechanics applies to coasting.
- **2D in the ecliptic plane first** (i=0, fold Ω into ω), but the struct
  stores all six elements so 3D stays open.
- Element data source: JPL "Approximate Positions of the Planets" /
  JPL Horizons. Note JPL publishes ϖ (longitude of perihelion) and L (mean
  longitude); convert: ω = ϖ − Ω, M₀ = L − ϖ.
- **μ (gravitational parameter) belongs to the central body** (bodies.rs),
  not to OrbitalElements. Propagation fns take both.
- Light delay: computed on demand from positions, never stored.
- Target accuracy: ~1,000 km error bars. Error budget is dominated by source
  element quality and the two-body assumption, not numerics.

## Conventions (enforced or strongly held)

- **Units: SI base units and radians internally, everywhere.** Meters,
  seconds, m³/s² for μ. Convert to AU/days/degrees only at display
  boundaries. This is the #1 correctness convention in the codebase.
- **f64 for all positions, velocities, time deltas.** f32 only at the
  render boundary.
- **Time:** hifitime `Epoch` used bare (no newtype). `time.rs` owns the
  SimClock (GM-advanced game clock, 12h/24h TimeStep enum), the J2000 epoch
  constant, and the Epoch → f64-seconds boundary conversion. orbits.rs takes
  f64 seconds, never Epochs.
- **Errors:** thiserror enums in lib crates, anyhow in bins.
- **Workspace inheritance for everything:** `[workspace.dependencies]`,
  `[workspace.package]` (edition = "2024", license = "MIT"),
  `[workspace.lints]`. Member crates opt in with `{ workspace = true }` /
  `edition.workspace = true` / `[lints] workspace = true`. Remember:
  workspace tables are catalogs, members must subscribe explicitly.
- **Testing:** known-answer tests against JPL Horizons data for propagation
  (e.g., propagate Earth 365.25 days, assert within tolerance). Kepler
  solver edge cases: M=0 → E=0; e=0 → E=M. Round-trip tests for
  elements ↔ state-vector conversions. CI runs cargo test --workspace.
- **Commits:** conventional-commit style (feat:, fix:, chore:).
- Any `#![allow(dead_code)]` present is temporary scaffolding mute — remove
  as modules gain real callers; don't add more without asking.

## CI

`.github/workflows/ci.yml`: checkout → dtolnay/rust-toolchain@stable
(rustfmt, clippy) → Swatinem/rust-cache → `cargo fmt --all -- --check` →
`cargo clippy --workspace --all-targets -- -D warnings` →
`cargo test --workspace`. Warnings are errors in CI. rust-toolchain.toml
pins stable + components.

## Current state (as of this briefing)

Done: workspace scaffold, README, MIT license, .gitignore (/target, .idea/,
data/*.db), rust-toolchain.toml, CI green, workspace inheritance wired,
OrbitalElements struct in sim-core/src/orbits.rs (six elements + hifitime
Epoch, serde derives, doc comments, SI/radians).

`time.rs` — complete with 4 unit tests. `Clock` (named Clock, not SimClock)
wrapping a private Epoch; `TimeStep` enum extended beyond the planned 12h/24h
with Round (15s), Minute, Hour for combat pacing; `seconds_since(epoch, t)`
as a free function; `J2000` as a `pub static LazyLock<Epoch>` — noon **TT**
(not UTC; the 64 s difference is ~1,900 km of Earth motion, over the error
budget). LazyLock because hifitime's Epoch constructors aren't const fn.
A test pins J2000 == 2000-01-01 11:58:55.816 UTC.

`bodies.rs` — μ constants for Sun through Pluto system + Luna, from JPL
DE440 (ssd.jpl.nasa.gov/astro_par.html), converted to m³/s². Doc comments
quote JPL's as-published km³/s² values for eyeball provenance. MU_SOL is
clippy-rounded to 17 sig figs (bit-identical to the full DE440 value).
Planet values are "system" GMs (include moons) EXCEPT Earth: JPL lists
GM_Earth and GM_Moon separately, so MU_EARTH is Earth alone. Two-body μ
for the Earth-Moon barycenter = MU_SOL + MU_EARTH + MU_LUNA. Never
compute μ as G·M.

`orbits.rs` — COMPLETE for coasting bodies, validated against JPL. Three
pure free functions, each taking `&OrbitalElements` where relevant:
- `propagate_mean_anomaly(&elements, mu, dt) -> f64` — M₀ + n·dt,
  rem_euclid into [0, 2π). dt is seconds since elements.epoch.
- `solve_kepler(ma, ecc) -> Result<f64, KeplerError>` — Newton from E₀=M,
  tol 1e-12 on the step, 30-iter cap. thiserror variants NotElliptical /
  NotConverged carry the offending inputs.
- `position_at(&elements, ecc_anomaly) -> DVec2` — meters, heliocentric
  ecliptic. Uses the direct perifocal form x′ = a(cos E − e),
  y′ = a√(1−e²)·sin E (no true anomaly needed for position), rotated by
  `DVec2::from_angle(ω).rotate(..)`.
The planned `elements.position_at(mu, dt)` convenience method chaining all
three is NOT yet written. 7 unit tests: solver edge cases, (M, e) grid
round-trip (prime step counts, deliberately), circle/periapsis/apoapsis
degenerate orbits, Horizons known-answer.

Horizons test (`test_earth_coasting_to_jpl_data_2d`): EMB relative to Sun
body center, DE441, JD 2451545.0 + {0, 121.75, 243.5, 365.25} days.
Observed misses: 0.15 m at dt=0 (transcription exact), growing to
~3,300–7,700 km across the year — the measured cost of the two-body
assumption (exceeds the 1,000 km budget within a year; accepted for
gameplay). Assert tolerance 1e7 m with that headroom. Horizons' printed
"Keplerian GM" matched MU_SOL + MU_EARTH + MU_LUNA to every digit. EMB z
stayed within ~360 km of the ecliptic all year — empirical license for 2D.

Next up, in order:
1. Commit the raw Horizons outputs to `crates/sim-core/testdata/`
   (horizons_emb_elements.txt, horizons_emb_vectors.txt) — the test's
   provenance comment already cites them, but they only exist in a
   scratch file so far. Receipt convention: raw dump committed verbatim,
   short query-fingerprint comment above the test, record the observed
   miss that justified the tolerance.
2. `vectors.rs` — state vector ↔ elements conversions. Test as round-trips
   through the solver, plus the J2000-epoch Horizons vector row (already
   in testdata) as a known answer.
3. Convenience entry point chaining propagate → solve → position, then
   the CLI plot/solve commands.

## Known gotchas (avoid repeats)

- `cargo new` inside the workspace runs its own `git init` if invoked before
  the root repo exists, and stamps literal `edition = "2024"`. Use
  `--vcs none` and convert to `edition.workspace = true`.
- RustRover: exactly ONE VCS root (Settings → Version Control → Directory
  Mappings). It has previously recreated nested repos from stale mappings.
  Trust `cargo check` and terminal git over IDE state when they disagree.
- Remote is `origin` on github.com/rwekyes/ship_sim; HTTPS auth via gh is
  the working path (SSH key exists but agent setup is unfinished).
- Branch is `master`.
- hifitime: `to_utc(Unit)` returns a raw f64 count since the 1900 reference,
  NOT a UTC Epoch — use `to_time_scale(TimeScale::UTC)`. Epoch's PartialEq
  compares the instant across time scales, so prefer comparing Epochs over
  Display strings in tests.
- Float division never panics in Rust — bad μ or a yields inf/NaN that
  propagates silently. Validate data at the load boundary (parse, don't
  validate); math fns trust inputs, debug_assert! as tripwire.
- Don't restate a constant's value in its doc comment — the copies drift
  (happened day one in bodies.rs). Docs carry provenance/units/what the
  code can't say; quote source-published values (km³/s²), not converted.
- hifitime::J2000_REF_EPOCH is noon **TAI**, not TT — 32.184 s (~950 km of
  Earth motion) off the project's J2000. Always use crate::time::J2000
  (bitten once: imported into a test before catching it).
- glam: Vec2/Mat2 are f32; the f64 types are DVec2/DMat2. Bare Vec2 must
  never appear in sim-core, and it's what autocomplete offers first.
- Test inputs with accidental structure hide bugs (bitten twice): (M, e)
  pairs with M == e can't catch swapped arguments, and dt ≈ one orbital
  period can't catch a no-op propagation. Use distinct values, awkward
  mid-orbit epochs, prime step counts in grids.
- A test fn without #[test] silently never runs (bitten once — two tests
  were inert while "passing"): read the by-name list in the runner output
  after adding tests. cargo test also captures stdout of passing tests —
  `cargo test -- --nocapture` (or --show-output) to see println!.
- Exact float == in tests only survives identical arithmetic routes; any
  comparison between different-but-equivalent computations (trig
  identities, rotations) needs a tolerance scaled to magnitude — f64 has
  ~16 significant digits, not decimal places, so 1e-12 absolute is
  meaningless at AU scale (~1 m is nine orders inside budget).
- JPL Horizons: tables are timestamped TDB (≈TT, sub-ms — ignorable,
  unlike TAI). Time-span fields accept Julian dates — use JD arithmetic,
  never calendar math (2000 was a leap year; 365.25 d after J2000 is
  2000-Dec-31 18:00, bitten once). Target "Earth" resolves to 399 — EMB
  is body "3". Center must be Sun body center (500@10), NOT the default
  solar-system barycenter @0 (~1.5M km apart, Jupiter's doing). Horizons
  ELEMENTS output gives W, OM, MA directly — the ϖ/L → ω/M₀ conversion
  only applies to the "Approximate Positions" table, not Horizons.
- Angles in OrbitalElements only feed sin/cos, so out-of-[0, 2π) values
  work, but keep them canonical on construction (rem_euclid) so tests and
  eyeballs agree.