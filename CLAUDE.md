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

Next up, in order:
1. `time.rs` — SimClock, TimeStep enum, J2000 constant, seconds_since().
2. `orbits.rs` — Kepler solver (M → E, Newton iteration) and E → ν →
   in-plane position. Test in isolation with textbook values.
3. `vectors.rs` — state vector ↔ elements conversions. Test as round-trips
   through the solver.
4. First Horizons known-answer test; then bodies.rs (Sol system hardcoded),
   then the CLI plot/solve commands.

## Known gotchas from setup (avoid repeats)

- `cargo new` inside the workspace runs its own `git init` if invoked before
  the root repo exists, and stamps literal `edition = "2024"`. Use
  `--vcs none` and convert to `edition.workspace = true`.
- RustRover: exactly ONE VCS root (Settings → Version Control → Directory
  Mappings). It has previously recreated nested repos from stale mappings.
  Trust `cargo check` and terminal git over IDE state when they disagree.
- Remote is `origin` on github.com/rwekyes/ship_sim; HTTPS auth via gh is
  the working path (SSH key exists but agent setup is unfinished).
- Branch is `master`.