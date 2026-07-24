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
- **Full 3D** (decided 2026-07-15, branch adventure-to-the-third-dimension;
  supersedes "2D first"). The 2D fold (i=0, Ω into ω) cost more than it
  saved — retrograde/inclination questions kept leaking in. DVec3
  throughout; perifocal → ecliptic via ZXZ rotation Rz(Ω)·Rx(i)·Rz(ω),
  factored into ONE shared helper used by both position and velocity.
  i is a polar angle, canonical range [0, π] — rem_euclid(TAU) is wrong
  for it (fine for Ω/ω/M).
- Element data source: JPL "Approximate Positions of the Planets" /
  JPL Horizons. Note JPL publishes ϖ (longitude of perihelion) and L (mean
  longitude); convert: ω = ϖ − Ω, M₀ = L − ϖ.
- **μ (gravitational parameter) belongs to the central body** (bodies.rs),
  not to OrbitalElements. Propagation fns take both.
- Light delay: computed on demand from positions, never stored.
- **Coasting state is a `Trajectory` enum** (vectors.rs), built by
  `Trajectory::from_state(&StateVector, mu, epoch)`: Elliptic(OrbitalElements),
  Escape(StateVector, Epoch), PureRadial(StateVector, Epoch). Not a
  Result — every physical outcome is a variant; escape/radial are valid
  answers, not errors. PureRadial is canon-motivated: ring gates are
  fixed relative to the star, so a ship can exit one with zero angular
  momentum. (A Retrograde variant existed briefly in 2D; in 3D retrograde
  is just i > π/2 inside the elements — deleted, don't reintroduce.)
  Escape/PureRadial carry the raw state vector + its Epoch (lossless;
  propagate numerically until hyperbolic/rectilinear Kepler exists).
  Classification order: |h|/(|r||v|) below tolerance → PureRadial
  (h = r.cross(v), test is sin of angle between r and v, scale-free);
  ε ≥ 0 → Escape; else Elliptic. Policy on variants (messaging,
  propagation strategy) lives in CLI/server, never sim-core. Matches must
  be exhaustive — no `_` arms, no #[non_exhaustive]; compiler-driven
  refactor is the upgrade path.
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

## Current state (as of 2026-07-23)

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

`orbits.rs` — COMPLETE for coasting bodies, FULLY 3D, validated against
JPL including an inclined body. Four pure free functions plus one private
helper:
- `propagate_mean_anomaly(&elements, mu, dt) -> f64` — M₀ + n·dt,
  rem_euclid into [0, 2π). dt is seconds since elements.epoch.
- `solve_kepler(ma, ecc) -> Result<f64, KeplerError>` — Newton from E₀=M,
  tol 1e-12 on the step, 30-iter cap. thiserror variants NotElliptical /
  NotConverged carry the offending inputs.
- `position_at(&elements, ecc_anomaly) -> DVec3` — meters, heliocentric
  ecliptic. Direct perifocal form x′ = a(cos E − e), y′ = a√(1−e²)·sin E,
  z′ = 0 (no true anomaly needed), rotated by the shared helper.
- `velocity_at(&elements, mu, ecc_anomaly) -> DVec3` — m/s;
  ẋ′ = −(n·a²/r)·sin E, ẏ′ = (n·a²/r)·√(1−e²)·cos E, same rotation.
  (The long-flagged y_prime bug was fixed 2026-07-16 before the 3D flip;
  moved here from vectors.rs.)
- `perifocal_to_ecliptic(&elements) -> DMat3` — PRIVATE; the ONE shared
  ZXZ rotation, `Rz(Ω) * Rx(i) * Rz(ω)` (glam right-to-left, ω applies
  first; a comment in code guards the order).
The planned `elements.position_at(mu, dt)` convenience method chaining
propagate → solve → position is NOT yet written. 9 unit tests: solver
edge cases, (M, e) grid (prime step counts), circle/periapsis/apoapsis,
two Horizons known answers. The circle test's alternate-route check was
reworked for 3D: fold only ω into the in-plane angle (argument of
latitude u = ω + E — the foldable rotation shares E's axis), then apply
Rz(Ω)·Rx(i) explicitly; different arithmetic route from production, so
it catches rotation-order bugs. Its elements use distinct nonzero i/Ω/ω
(0.1 / 0.333 / 0.777). Periapsis/apoapsis tests keep zero angles
deliberately — they assert only |r|, which is rotation-invariant.

Horizons known-answer tests, both 3D (IN/OM/W fed as published, each
individually `.to_radians()`; expected vectors carry the z column):
- `test_earth_coasting_to_jpl_data` — EMB relative to Sun body center,
  DE441, JD 2451545.0 + {0, 121.75, 243.5, 365.25} days. Misses 0.15 m
  at dt=0 (transcription exact) growing to ~3,300–7,700 km over the year
  (two-body cost; exceeds the 1,000 km budget within a year; accepted
  for gameplay). Tolerance 1e7 m. Horizons "Keplerian GM" ==
  MU_SOL + MU_EARTH + MU_LUNA to every digit.
- `test_pallas_coasting_to_jpl_data` — 2 Pallas (JPL#74 small-body
  solution), same span/step/center. Chosen over Mercury for inclination
  signal: i ≈ 34.85° vs 7°. (Mercury's GR perihelion precession was
  raised as a concern but is ~120 km/yr — negligible; even its Newtonian
  perturbations are only ~12× that. Inclination coverage was the real
  criterion.) μ = MU_SOL alone; Keplerian GM matched it to every printed
  digit. Measured miss profile: 0 / 6,041 / 22,235 / 43,135 km across
  the year — Jupiter perturbation, ~6× the EMB cost, superlinear because
  the drifting semi-major axis compounds along-track. Tolerance 6e7 m
  (~1.4× headroom, same ratio as the EMB test). Debugging cross-check
  worth remembering: each Horizons row's own osculating elements
  reproduce that instant's truth vector to sub-meter at i=34.8°, which
  validates the ZXZ geometry independent of the dynamics.
- Campaign-data consequence: belt objects blow the 1,000 km budget ~40×
  within a year of a frozen element set. When seeding `data/`, asteroid
  elements want an epoch near the campaign's game date; planets are far
  more forgiving.

Raw Horizons outputs committed at `crates/sim-core/test_data/`
(horizons_{emb,pallas}_{elements,vectors}.txt). Receipt convention: raw
dump committed verbatim, short query-fingerprint comment above the test,
record the observed miss that justified the tolerance.

`vectors.rs` — COMPLETE. StateVector (DVec3 position/velocity, derives +
serde), Trajectory enum (Elliptic/Escape/PureRadial; Retrograde deleted
per domain decisions), `elements_to_state_vector(&elements, mu, E)`
composing position_at + velocity_at, and `Trajectory::from_state` in
full 3D. Module split is by frame: orbits.rs owns the elements world
(both perifocal functions + the rotation helper, which stays private
there); vectors.rs owns the Cartesian world and the conversions crossing
the boundary.

`Trajectory::from_state(state, mu, epoch) -> Trajectory` (takes StateVector
by value, not &): h = r.cross(v); classify by |h|/(|r||v|) ≤ 1e-8 →
PureRadial, then ε ≥ 0 → Escape, else Elliptic (radial check FIRST, before
escape — a collinear state resolves to PureRadial regardless of energy).
Element recovery: node n = ẑ×h; ecc_vec = v×h/μ − r̂; i = acos(h_z/|h|)
clamped [−1,1]; a = −μ/2ε. Singularity handling via two guards — equatorial
(i < 1e-8) swaps node_dir to DVec3::X, circular (e < 1e-8) swaps peri_dir
to node_dir. ω = 0 in the circular case (convention); when circular, the
mean anomaly slot instead carries the argument of latitude measured
node→r (ω folded into M). A private `angle_in_plane(from, to, h_hat)`
helper does the signed-angle-about-an-axis work (cross·axis atan2 dot,
rem_euclid TAU), reused for Ω, ω, and the circular M. Non-circular M
recovered via E from cos_e/sin_e → atan2 → M = E − e·sin E.

Tests (7, all pass): two Horizons elements→state→elements round-trips
(test_earth_round_trip, test_pallas_round_trip — 4 sample points each,
prime-ish step, reusing the shared test_round_trip helper) plus five
edge cases: test_circular, test_circular_equatorial, test_equatorial
(round-trip position/velocity within 1e3 m / 1e1 m/s), test_escape
(50 km/s tangential at 1 AU, matches! Escape), test_pure_radial (v = 2e-7·r,
a 3D non-axis-aligned collinear pair, matches! PureRadial). Round-trip
gotcha baked into the edge-case tests: once ω is folded into M you must
RE-SOLVE Kepler from the reconstructed elements' mean anomaly before
rebuilding the state — reusing the original E puts a circular body at the
wrong argument of latitude (the bug that stalled test_circular).

3D migration (branch adventure-to-the-third-dimension): COMPLETE.
Checklist items 1–7 plus `Trajectory::from_state` in full 3D all done.

Clippy/fmt/tests all green as of 2026-07-23.

Next up, in order:
1. Convenience entry point chaining propagate → solve → position, then
   the CLI plot/solve commands.
2. PINNED FEATURE (2026-07-17, wanted after the thrust integrator):
   perturbed-coast propagation via special perturbations — reuse the
   powered-flight numerical integrator with acceleration = solar
   two-body + Σ over perturbers of μ_k·(direct − indirect) terms;
   perturber positions come from existing Kepler propagation (analytic
   rails, hierarchy makes it cheap). The indirect term (the perturber
   also accelerates the heliocentric origin) is mandatory — omitting it
   wrongs the correction by roughly its own size. Jupiter first
   (~43,000 km/yr on Pallas), Saturn is a few percent of Jupiter,
   Uranus/Neptune noise. Truncate the perturber sum by error budget,
   same method as test tolerances: measure, compare, keep what matters.

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
- glam: Vec2/Vec3/Mat2/Mat3/Quat are all f32; the f64 types are the
  D-prefixed ones (DVec3/DMat3/DQuat). Bare f32 types must never appear
  in sim-core, and they're what autocomplete offers first.
- A rotation-order bug in the ZXZ perifocal→ecliptic transform is
  invisible to every zero-inclination test. Only an inclined-body known
  answer catches Rz/Rx composed backwards. glam composes right-to-left
  (a * b applies b first).
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
- Horizons transcription (bitten 2026-07-17, three ways at once): a
  dropped exponent sign turns `E-04` into `e4` (Earth's IN became 10,346°);
  EVERY angle field needs its own `.to_radians()` — the two added for 3D
  (IN, OM) shipped as raw degrees while their neighbors were converted;
  and the 2D fold is dead — `arg_periapsis` is W as published, never
  OM + W (the rotation matrix applies Ω itself; summing double-counts it).
  The dt=0 row hitting to sub-meter is the transcription check; bad
  angles don't error, they just propagate silently to a spectacular miss.
- Small-body Horizons files print the orbit-solution elements (at the
  solution's own epoch) in the HEADER — transcribe from the $$SOE table
  rows, not the header block. Provenance is a JPL#N solution ID, not a
  DE ephemeris number.