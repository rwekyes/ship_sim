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
- **Burn PLANNER and burn INTEGRATOR are two different pieces of code**
  (settled 2026-08-08 — these were previously one confused bullet).
  The *planner* answers "how long do I burn, when do I flip, how much Δv"
  and is closed-form kinematics; this is where "at sustained ~0.3g solar
  gravity is negligible for powered legs" applies. The *integrator*
  answers "given thrust, where is the ship at t+dt" and its acceleration
  is ALWAYS gravity + thrust, never thrust alone. Do not write a
  thrust-only kinematic stepper: the pinned perturbation feature reuses
  the integrator for pure coasting, which is only possible if the
  acceleration term is pluggable.
- **Expanse travel = brachistochrone burns** (accelerate, flip, decelerate).
  Orbital mechanics applies to coasting.
- **Numerical integration only for ships under thrust** (plus the pinned
  perturbed-coast feature). Semi-implicit Euler first, RK4 later. Burns
  subdivide internally (~60s substeps) within the 12h/24h game timesteps.
  After a burn, convert state vector back to elements via
  `Trajectory::from_state`. Semi-implicit ordering is `v += a·dt` FIRST,
  then `r += v_new·dt` — using the old v is plain forward Euler and gains
  energy secularly (orbits spiral outward). It passes short-burn tests and
  fails long coasts; the energy-bound test below is what catches it.
- **Thrust is commanded ACCELERATION, not force + mass** (settled
  2026-08-08). Expanse ships are specified as "burning at a third of a g"
  and players talk that way. Force/mass drags propellant mass in as a
  state variable and makes the ODE non-separable for no gameplay gain.
  Fuel burn can ride alongside later as a decoupled scalar
  (ṁ = m·a/(g₀·Isp) at constant commanded accel) without feeding back
  into the dynamics.
- **Thrust direction is a fixed inertial DVec3** for now (settled
  2026-08-08), normalized on construction, not per substep. That is
  physically what a brachistochrone leg does — hold one inertial heading.
  A direction-policy enum (Prograde/TowardPoint/…) is the upgrade path,
  but note it makes acceleration velocity-dependent and forfeits the
  symplectic property the energy test relies on.
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
- **Test DRY rule (settled 2026-08-09): duplicate assertions and expected
  values; extract fixtures and setup.** An expected value computed by a
  shared helper can hide a bug — the helper's error cancels the code's and
  the test goes green while lying. So known-answer data (Horizons vectors)
  stays literal and local in each test, even across several tests. Setup
  with no assertion semantics — transcribed element structs, "build state,
  integrate, measure" plumbing — gets a helper, because a re-transcription
  that updates one copy and not the other makes two tests silently disagree
  about what orbit they are testing. Applied in `integrate.rs`:
  `emb_j2000_elements` / `kepler_miss` extracted, `coast_vs_horizons`
  vectors left inline.
- **Prefer asserting a RELATIONSHIP over a second absolute tolerance** when
  the claim is about a relationship. A convergence test that just asserts
  `miss < some_number` at a smaller step is a second magnitude test wearing
  the wrong name — it passes at 0.66× the error, which would mean the
  scheme is not first-order. Assert the ratio.
- **Commits:** conventional-commit style (feat:, fix:, chore:).
- Any `#![allow(dead_code)]` present is temporary scaffolding mute — remove
  as modules gain real callers; don't add more without asking.

## CI

`.github/workflows/ci.yml`: checkout → dtolnay/rust-toolchain@stable
(rustfmt, clippy) → Swatinem/rust-cache → `cargo fmt --all -- --check` →
`cargo clippy --workspace --all-targets -- -D warnings` →
`cargo test --workspace`. Warnings are errors in CI. rust-toolchain.toml
pins stable + components.

## Current state (as of 2026-08-10)

Head is 0a4420c on branch `burn_integrator`, pushed, working tree clean.
36 sim-core tests, fmt/clippy/tests all green locally (full CI gate run,
not just `cargo test`). `burns.rs` is the newest module and roadmap item 3
is now COMPLETE — the `Burn` type, its validating constructor, the serde
boundary, and `accel_at` all exist and are tested. Roadmap item 2
(`integrate.rs`) is COMPLETE — the integrator is validated four independent
ways: an exact analytic oracle (a), a convergence rate (b), a conservation
law (c), and a closed-form error model (d). Nothing in sim-core has yet
CALLED `accel_at` from inside `integrate`; that wiring is the first thing
item 4 will touch.


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

`orbits.rs` addition (2026-08-08): `OrbitalElements::position_at_dt(self,
mu, dt) -> Result<DVec3, KeplerError>` — the convenience chain
propagate → solve → position. Takes `self` by value (OrbitalElements is
Copy, so callers keep theirs) and passes `&self` inward. Uses `?` on
solve_kepler to propagate KeplerError; calls `position_at` directly, NOT
`elements_to_state_vector` — the vectors.rs import that briefly existed
here was removed, module split intact. A `state_vector_at_dt` sibling is
still NOT written; the integrator tests build their initial state via
`elements_to_state_vector` in the test body instead.

`orbits.rs` addition (2026-08-09): `OrbitalElements::mean_motion(self, mu)
-> f64` = `(mu / a³).sqrt()`, and `OrbitalElements::period(self, mu) -> f64`
= `TAU / mean_motion`. Kepler's third law, one home. `mean_motion` was
previously written inline in TWO places — `propagate_mean_anomaly` and
`velocity_at` — and the energy test wanted it a third time; both call sites
now go through the method. `self` by value matches `position_at_dt`; μ stays
a parameter, never a field (central-body rule). Swapping both sites moved no
test, which is the check that the two inline copies really were the same
expression.
- DOC DRIFT to fix: the units explanation ("μ is m³/s², a³ is m³, so μ/a³
  is 1/s² and its square root is rad/s") still sits on
  `propagate_mean_anomaly`, but the arithmetic it explains now lives in
  `mean_motion`. Move it. Neither new method has a doc comment yet, which
  breaks the file's convention that every pub fn carries one.

`integrate.rs` — NEW 2026-08-08, COMPLETE, 4 tests passing — the full
(a)/(b)/(c)/(d) set from the plan.
- `two_body(mu, &state) -> DVec3` — `-mu * r / r.length().powi(3)`.
  The cube is inverse-square PLUS the normalization of r, not
  inverse-cube gravity. Sign is negative because r runs origin→ship and
  gravity pulls back down it. Ship mass cancels out of F = GMm/r², which
  is why this composes by plain addition with commanded-acceleration
  thrust. Guarded by `debug_assert!(r.is_finite() && r.length() > 1.0)`.
- `integrate<F>(mut state, t0, dt_total, substep, accel: F) -> StateVector
  where F: Fn(f64, &StateVector) -> DVec3` — semi-implicit Euler.
  `n = (dt_total/substep).ceil()`, early return if n == 0.0 (also dodges
  the h = 0.0/0.0 = NaN case), `h = dt_total/n` so steps are uniform and
  land exactly on t0 + dt_total. `t` is computed fresh at the TOP of each
  iteration as `t0 + i as f64 * h` — a loop-local `let`, not an outer
  `mut` accumulator (avoids drift AND the unused_assignments lint).
  Guarded by `debug_assert!(dt_total >= 0.0 && substep > 0.0)`.
- Tests, all on a 121.75 d span (175,320 min), all with receipts recorded
  2026-08-09:
  - `integrate_vs_kepler` — integrator vs `position_at_dt` on the same EMB
    elements. Kepler is the EXACT analytic solution to the ODE being
    integrated, so the miss is PURE truncation error, isolated and
    attributable. 60 s substeps, observed miss 2,861,969.42 m, tolerance
    3e6.
  - `coast_vs_horizons` — end-to-end vs DE441 truth, which conflates
    two-body model error with truncation; kept as a separate named test
    precisely so a failure says which layer moved. 60 s substeps, observed
    miss 481,882.79 m, tolerance 5e5. NOTE it is SMALLER than the pure
    truncation miss above — the two error sources partly cancel here, so
    this test is not a bound on either one alone. That is exactly why both
    exist.
  - `step_size_convergence` — test (b). Asserts the RATIO
    `kepler_miss(30.0) / kepler_miss(60.0)` lands in 0.48..=0.52, not a
    second absolute tolerance. Observed ratio 0.5000010 (1,430,987.64 m
    vs half of 2,861,969.42 = 1,430,984.71, off by 2.93 m) — ~40×
    headroom inside the band, while a second-order scheme (0.25) or a
    broken one (→1.0) fails loudly.
  - `energy_bound` — test (c), added 2026-08-09. ZERO THRUST. Asserts
    `max |ε − ε₀| / |ε₀| < 2e-5` where `ε = v²/2 − μ/r`, over 100 orbits
    sampled **20 times per orbit**, 3600 s substeps, `dt` derived from
    `elements.period(mu)`. Observed max drift 1.279973e-5 (~56% headroom).
    Verified to actually catch its bug: swapping the two update lines to
    forward Euler gives 0.354, a factor of 2.8e4. Both numbers recorded in
    the test comment. Independently confirmed the oscillation amplitude is
    FLAT across 400 orbits (1.279376e-5 → 1.279380e-5, seventh sig fig) —
    that flatness, not the position tests, is the actual proof the stepper
    is symplectic.
    - The sampling rate is load-bearing; see the sampling-phase gotcha
      below. Do NOT "simplify" this to one sample per orbit.
    - `t0` accumulates across chunks and is passed into `integrate`. It is
      a no-op today (gravity ignores t) and exists so the pattern is right
      before `burns.rs` makes it load-bearing.
  - `pure_kinematics` — test (d), added 2026-08-09. μ = 0, acceleration is
    a constant-vector closure `|_t, _s| a` (NOT `two_body` — the term is
    identically zero and there is nowhere to put μ in a bare fn anyway).
    Not a tolerance test: semi-implicit Euler's error for constant accel is
    EXACT and closed-form, so the test asserts the full expression
    `r_N = r0 + v0*t + 0.5*a*t^2 + 0.5*a*h*t`. That last term is the Euler
    bias; it is proportional to h, which is why the test must recompute
    `h = dt/ceil(dt/substep)` rather than assume `h == substep`.
    Inputs deliberately awkward: `a = (0.33, -2.1, 7.7)` non-axis-aligned,
    `v0 = (0.2, 9.9, 5.3)` all-nonzero (so the `v0*t` term is actually
    exercised), `dt = 333` / `substep = 13` which does NOT divide evenly —
    that combination is the only coverage the `ceil` path has.
    Observed position residual 3.0517578125222045e-5 m, which is EXACTLY
    one ULP at the y coordinate (1.4469e11, ULP = 2^-15 = 3.0517578125e-5).
    The closed form reproduces the integrator bit-for-bit; the residual is
    the float floor, not an approximation.
    Velocity is separately asserted: for constant accel `v_N = v0 + a*t`
    with NO bias term — the correction lands only on position. Observed
    9.183e-13 m/s against a 1e-9 tolerance.
    - RESOLVED 2026-08-10: the position tolerance was 3.052e-5, i.e.
      1.00008× the observed value — no headroom at all. Two ULP (6.1e-5)
      fails, and a release build's FMA contraction or a larger fixture
      coordinate would get there. Now widened to 1e-4 (~3 ULP), still nine
      orders inside the 1,000 km budget. Same knife-edge mistake as the
      first `energy_bound` threshold; see the loop-bound tolerance gotcha
      below. NOTE the assert's failure MESSAGE still reads "above tolerance
      3.052e-5" while the compared value is 1e-4 — stale string, would
      misreport on failure. Fix when next in the file.
    - Bonus guard, not yet recorded as a receipt: forward Euler flips the
      bias SIGN, so the two schemes differ by exactly `a*h*t` = 34,068.7 m
      here. This test discriminates the statement ordering by nine orders
      of magnitude — a sharper signal than `energy_bound` gives, because
      the expected value is exact rather than a bound.
- Test-module helpers (extracted 2026-08-09, commit dced990):
  `emb_j2000_elements() -> OrbitalElements` (the transcribed JPL fixture)
  and `kepler_miss(substep: f64) -> f64` (build state → integrate →
  distance vs `position_at_dt`). Extracted because the two Kepler-oracle
  tests are the SAME experiment at two step sizes; duplicating transcribed
  Horizons data across tests is the failure mode this repo has already been
  bitten by three ways. `coast_vs_horizons` deliberately keeps its expected
  vectors inline and does NOT use the helpers — see the testing convention
  below. `specific_energy(mu, &state) -> f64` is a third helper, test-local
  for now; `Trajectory::from_state` already computes ε internally for its
  Escape/Elliptic split, so promoting it into `vectors.rs` is the obvious
  next dedup if a third caller shows up.

`burns.rs` — NEW 2026-08-10, COMPLETE (roadmap item 3). The type, its
validating constructor, the serde boundary, and `accel_at` are all done and
tested. Commits 0daf0a8 → 0a4420c on branch `burn_integrator`.
- `BurnError` — thiserror enum, three variants, each carrying the offending
  value: `InvalidDirection(DVec3)`, `InvalidDuration(f64)`,
  `InvalidAccel(f64)`. Deliberately does NOT derive `PartialEq` — the
  variants carry f64, and `NaN != NaN`, so `assert_eq!` on an error built
  from a NaN input would fail for reasons unrelated to the code. Tests
  match with `let Err(BurnError::X(v)) = … else { panic!() }` instead,
  which also binds the payload so the carried value gets asserted.
- `Burn` — `start: Epoch`, `duration: f64` (seconds), `accel: f64` (m/s²),
  `direction: DVec3` (unit). ALL FOUR FIELDS PRIVATE, four by-value getters
  (`self`, not `&self` — matches `position_at_dt`/`mean_motion`; the type
  is Copy).
- **Acceleration is a SCALAR plus a separate unit direction, not one
  combined vector** (settled 2026-08-10). The two are constrained by
  different things: magnitude by the SHIP (max thrust, crew g-tolerance),
  direction by the TRAJECTORY solve. Splitting them gives the magnitude a
  natural place to be range-checked and leaves `direction` with exactly one
  invariant. It also matches the planner's output shape (Δv, burn time,
  heading), the display boundary (players say "a third of a g"), and the
  future fuel scalar ṁ = m·a/(g₀·Isp), which wants the magnitude directly
  rather than a `.length()` re-derivation.
- There is deliberately NO maximum-accel check. That is a ship property,
  not a burn property — `Burn` cannot know whether 5 g is impossible or
  merely unpleasant. Same rule as μ belonging to the central body: the
  constraint lives with the thing that owns it. It goes wherever `Ship`
  lands.
- **Zero accel and zero duration are VALID, not errors** (settled
  2026-08-10). A degenerate planner leg (a burn that rounds away, a flip
  with no coast) should not be an error, and `accel_at` returns zero for it
  anyway. The bar is finite and non-negative. `zeros_are_ok` exists purely
  to pin this judgment call so a later "tightening" to `> 0.0` goes red.
- `Burn::new(start, duration, accel, direction) -> Result<Self, BurnError>`
  — parse-don't-validate. `direction.try_normalize().ok_or(…)?` handles
  zero AND non-finite vectors in one shot (`try_normalize` returns None for
  both; bare `normalize` would silently yield NaN). The f64 guards are
  written `!x.is_finite() || x < 0.0` so NaN falls into the error branch —
  `x >= 0.0` inverted does not, as obviously. The shadowed `let direction`
  means the rest of the fn cannot see the un-normalized parameter.
- **serde: `#[serde(try_from = "BurnRepr")]`.** A plain `#[derive(Deserialize)]`
  builds structs field-by-field and NEVER calls the constructor — field
  privacy does not stop it — so seed JSON with `"direction":[3,0,0]` would
  have given 3× thrust silently. `BurnRepr` is a private mirror struct
  deriving only `Deserialize`; `impl TryFrom<BurnRepr> for Burn` just calls
  `Burn::new`. One validation path for both code and wire.
  - The attribute takes the type name as a STRING LITERAL (serde quirk).
  - It requires `Self::Error: Display`, which thiserror's `#[error("…")]`
    already provides — the concrete payoff for using thiserror here.
  - `Serialize` still reads `Burn`'s own fields while `Deserialize` reads
    `BurnRepr`'s, so the two field-name lists must agree. Renaming one and
    not the other breaks the round-trip at RUNTIME. Adding a field is safe
    (BurnRepr feeds `new` positionally, so arity changes are a compile
    error). `serialize_round_trip` is the guard for the rename case.
  - Typed error is LOST across the boundary: serde folds `BurnError` into
    `serde_json::Error`, keeping only the Display string. Callers
    deserializing seed data cannot `match` on `BurnError`. Relevant when
    player-command handling gets built.
- `accel_at(self, reference: Epoch, t: f64) -> DVec3` — the thrust HALF of
  the acceleration only; callers add gravity. Body is four lines:
  `debug_assert!(t.is_finite())`, `start_s = seconds_since(reference,
  self.start)`, `end_s = start_s + self.duration`, then
  `if (start_s..end_s).contains(&t) { self.accel * self.direction } else
  { DVec3::ZERO }`.
  - **`reference` is the Epoch that `t == 0.0` means.** `Burn` stores an
    absolute `start: Epoch` (survives the DB and the wire); `integrate`
    works in bare f64 seconds whose origin nothing records. `reference` is
    that origin. THE CALLER MUST PASS THE SAME `reference` IT USED TO BUILD
    `t0` — that is the one invariant this design cannot check for itself,
    so comment it at the call site.
  - Converts the BURN into f64, never `t` back into an `Epoch`. One Epoch
    conversion per call instead of hifitime arithmetic on every substep.
  - `(a..b)` is half-open BY CONSTRUCTION, so the window decision lives in
    the type rather than in a remembered `<` vs `<=`. It also dodges
    `clippy::manual_range_contains`, which rejects the hand-written form.
  - Falls out for free, both verified: a zero-duration burn NEVER fires
    (`(x..x)` is empty — correct, zero duration is zero impulse), and a NaN
    `t` returns `DVec3::ZERO` (all NaN comparisons are false). The latter
    is why the `debug_assert` is there: a silently-not-firing burn looks
    exactly like a correctly-outside-the-window one.
  - Because the constructor normalized `direction`, the returned vector's
    magnitude is EXACTLY `self.accel`. That is the whole payoff of
    normalizing at construction — an unnormalized direction would scale
    thrust here silently.
- 12 tests. Constructor/serde (7): `serialize_round_trip`, `non_unit_json`,
  `invalid_direction`, `invalid_duration`, `invalid_accel`, `zeros_are_ok`,
  `serde_rejection`. `accel_at` (5): `fires_inside_window`,
  `zero_before_and_after`, `boundary_instants`,
  `reference_shift_invariance`, `zero_duration_never_fires`, sharing a
  `test_burn()` fixture (start = J2000, duration 900 s, accel 3.2675,
  direction (2,−3,6)).
  - `boundary_instants` is the only test that pins the half-open decision:
    `t = 0` fires, `t = 899` fires, `t = 900` does NOT. Every other test
    passes with either a half-open or a closed window.
  - `fires_inside_window` computes its expected value as
    `DVec3::new(2,-3,6) / 7.0 * accel` — deliberately NOT
    `.normalize() * accel`, which would route the expected value through
    the same function under test and cancel a normalization bug. 7 is exact
    because 49 is a perfect square, which is why (2,−3,6) was chosen.
    Tolerance 1e-12, scaled to the ~3.27 magnitude (not the 1e-15 used for
    unit-vector comparisons elsewhere in the file).
  - `reference_shift_invariance` is a PROPERTY test: the answer must depend
    only on the absolute instant `reference + t`, so the same instant
    queried through two different references must agree. Exact `==` is
    correct here (the output is SELECTED from two fixed vectors, never
    computed from `t`, so there is no float error to tolerate). It catches
    two bugs nothing else does — hardcoding `*J2000` instead of using the
    `reference` parameter, and flipped `seconds_since` arguments. It also
    feeds a NEGATIVE `t` (via `ref_b` after the burn), the only test that
    does. Currently samples ONE instant; sampling several (inside, before,
    after, the `start` boundary) would strengthen it.
  `serde_json` added to sim-core `[dev-dependencies]` (test-only, so it
  stays out of the shipped lib and the WASM build — the no-I/O rule is
  about what the crate IS, not what its tests use).
  - `non_unit_json` is the ONLY test that proves `try_from` is wired:
    feeds raw JSON `"direction":[2.0,-3.0,6.0]` (length exactly 7, so the
    expected value is exactly `(2,-3,6)/7`), asserts the result is
    normalized. Receipt: commenting out the attribute makes it fail while
    `serialize_round_trip` stays green. Chosen over `[3,0,0]` because an
    axis-aligned vector normalizes to itself and can pass by accident.
  - `serialize_round_trip` would pass WITH OR WITHOUT the attribute — what
    serde writes is already normalized, so a plain derive round-trips it
    unchanged. It is a field-name-drift guard, not invariant coverage.
  - The three `invalid_*` tests are near-identical blocks and deliberately
    do NOT share a helper — they are assertions, and the DRY rule below
    says assertions duplicate. Each covers negative/NaN/infinity (or
    zero/NaN/infinity for direction). The NaN cases are the point: a NaN
    accel is exactly what the original `!accel.is_nan()` typo let through
    while REJECTING every valid burn (found 2026-08-10 — see gotcha below).

3D migration (branch adventure-to-the-third-dimension): COMPLETE.
Checklist items 1–7 plus `Trajectory::from_state` in full 3D all done.

fmt/clippy/tests all green as of 2026-08-10 (36 sim-core tests).

Next up, in order (REORDERED 2026-08-08 — the burn integrator was moved
ahead of the CLI). Rationale: the CLI's command surface is defined by what
sim-core can do, so building a coast-only nav CLI means building it twice.
The usual "I need to see numbers to debug the physics" argument for a CLI
first does not apply here — the integrator has an exact oracle sitting next
to it in orbits.rs (turn thrust off, it must reproduce Kepler propagation),
so no plot and no new Horizons pull is needed to validate it.

DONE 2026-08-08: `position_at_dt` (item 1) and `integrate.rs` first pass
with test (a) in both flavours (item 2) — see the Current state section.
DONE 2026-08-09: tolerance receipts (old item 1) and test (b),
`step_size_convergence`. Euler's first order is now CONFIRMED, not assumed:
measured ratio 0.5000010. Consequence for the RK4 decision — halving the
substep buys exactly one halving of error, so 60 s → 1 s (60× the compute)
only buys 60×, leaving ~48 km of truncation over 121.75 d. Euler is fine
for burns (hours, not months); RK4 is what the pinned perturbed-coast
feature will need, not the burn integrator.

DONE 2026-08-09: test (c), `energy_bound`, plus the `mean_motion`/`period`
extraction it motivated. The integrator is now confirmed symplectic by
measurement (bounded energy oscillation, flat over 400 orbits), not by
inspection of the statement order.
DONE 2026-08-09: test (d), `pure_kinematics`. Roadmap item 2 is COMPLETE.
The plan called for asserting with a tolerance because "Euler carries a
½a·h·t bias"; that undersold it — for constant acceleration the bias is
EXACT, so (d) became a closed-form known-answer test hitting one ULP
rather than a loose sanity check. Only remaining chore is widening its
position tolerance (see the OUTSTANDING note in the Current state section).
   Measured Euler cost at 60 s substeps: negligible over a multi-day burn
   (thousands of steps); order 10⁴ km/yr along-track drift over year-long
   coasts, i.e. the same ballpark as the two-body error already accepted.
   Test (b) pinned the real number.
DONE 2026-08-10: item 3, ALL of it — the `Burn` type, `Burn::new`,
`BurnError`, the getters, the `#[serde(try_from)]` boundary, and
`accel_at`, with 12 tests. See the `burns.rs` entry in Current state. The
window is HALF-OPEN, `[start, start + duration)`: a closed interval
double-counts the shared instant when two brachistochrone legs abut, which
is exactly the flip point.

NOT YET DONE, and the first thing to do next: actually feed `accel_at` to
`integrate`. The closure is
`|t, s| two_body(mu, s) + burn.accel_at(ref_epoch, t)` — this is where
`integrate`'s `t0` threading stops being a no-op (gravity ignores time,
thrust does not), and where passing `two_body` bare would go wrong in a new
way. Acceleration stays gravity + thrust, NEVER thrust alone. A powered-leg
test wants an oracle: with μ = 0 the whole burn is `pure_kinematics` with a
window, so the closed-form `r_N = r0 + v0*t + 0.5*a*t² + 0.5*a*h*t` still
applies over the powered span and can be checked exactly.
4. The burn planner (closed-form; solve flip time / duration / Δv for a
   target). Deferred deliberately — it needs a trustworthy integrator to
   check against, and it does not need to exist for 1–3 to be correct.
5. CLI plot/solve commands.
6. PINNED FEATURE (2026-07-17, wanted after the thrust integrator):
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
- **Passing `two_body` bare to `integrate` COMPILES AND IS SILENTLY
  WRONG** (verified 2026-08-08). `two_body` is
  `fn(f64, &StateVector) -> DVec3`; the bound is
  `Fn(f64, &StateVector) -> DVec3`. Structurally identical, so the
  compiler accepts it — but the leading f64 means μ to one and t to the
  other, so gravity gets scaled by the elapsed second count. No error, no
  warning, wildly wrong answers. ALWAYS go through the closure:
  `|_t, s| two_body(mu, s)`. The types match; the meanings do not.
- Closures: `two_body(mu, &state)` CALLS it and yields a DVec3;
  `|_t, s| two_body(mu, s)` passes a recipe to be re-run at each new
  position. The integrator needs the recipe — acceleration changes as the
  ship moves. The closure is also where captured config (μ) lives, which
  is precisely why a plain `fn` cannot do the job: a fn has no
  environment, so there is nowhere to put μ.
- Unused CLOSURE parameters trip `unused_variables` exactly like function
  ones, and CI is `-D warnings` — gravity ignores time, so it is `_t`.
- `unused_assignments` ("value assigned to `t` is never read") is NOT
  `unused_variables`. It means the INITIAL value is dead, typically from
  keeping an outer `let mut t = t0;` while reassigning at the top of the
  loop. Fix by deleting the outer binding and making it a loop-local
  `let` — which also kills the accumulated-rounding question for free.
- Float→int `as` casts SATURATE, they do not panic, and NaN casts to 0.
  Measured for `(dt_total/substep).ceil() as u32`: negative → 0 and
  NaN → 0 (both a SILENT no-op that looks exactly like a correct
  zero-length step), substep 0.0 → inf → u32::MAX → 4.3 billion
  iterations at h=0, i.e. a hang. All four are why `integrate` guards its
  span with a debug_assert; `NaN >= 0.0` is false so one comparison
  catches NaN too, same trick as the `two_body` guard.
- `clippy::excessive_precision` on a Horizons literal is almost always a
  TRAILING ZERO (bitten three times now: MU_SOL, the vectors.rs velocity,
  the integrate.rs fixture). f64 holds ~15–17 significant digits; the
  trailing zero changes no bits, so trim it. Faster fix: the de-zeroed
  EMB values already exist in `vectors.rs` `test_earth_round_trip` —
  copy from there rather than re-editing raw Horizons output. Take the
  digit trim, not clippy's underscore-separated suggestion; the rest of
  the repo uses the plain form.
- **Sampling phase can cancel the very signal you are measuring** (bitten
  2026-08-09, `energy_bound`). Sampling a periodic system once per period
  means every sample lands at the SAME phase as the reference sample, so a
  phase-dependent quantity subtracts against itself. The first version of
  `energy_bound` sampled once per orbit and measured 8.4e-9 — the real
  oscillation amplitude is 1.28e-5, ~1,500× larger. Worse, the residual it
  did see grew LINEARLY with orbit count (8.387e-11 per orbit, dead linear
  from 1 to 2000 orbits), because the numerical period differs slightly
  from the Kepler period so the sample point slowly precesses. That looks
  exactly like secular energy drift and is not. Sample several times per
  period. Diagnostic that separates the two: fine-sample and check whether
  the AMPLITUDE is flat in time — bounded amplitude with a growing
  stroboscopic reading is phase walk, not drift.
- **An absolute tolerance cannot go below one ULP of the largest coordinate
  involved**, and ULP scales with magnitude: at 1.45e11 m one ULP is
  3.05e-5 m, at 1 AU-ish scale generally ~1e-5 m. `pure_kinematics` lands
  exactly on that floor. Two consequences: a residual equal to 1 ULP means
  the formula is EXACT (not "close"), and a tolerance set at the observed
  value has zero headroom because the next representable value up is 2×.
  Set such tolerances at a few ULP, never at the measurement.
- Receipts are for setting tolerances WITH headroom, not for setting them
  AT the observation (bitten twice now: `energy_bound`'s first threshold at
  7%, `pure_kinematics` at 1.00008×). Measure, then round up a few×. The
  repo's other tolerances run 1.4× (Horizons), 3.3× (ULP-floor tests) and
  40× (convergence ratio) — pick by how stable the quantity is, and treat
  anything under ~2× as a future false failure.
- A tolerance on a quantity that grows with a loop bound is a trap: the
  one-sample-per-orbit `energy_bound` had 7% headroom and would have failed
  at 108 orbits, blaming the physics for a threshold problem. Prefer
  asserting a quantity that is BOUNDED in the loop variable, so the
  tolerance survives someone lengthening the run.
- An accumulator whose only consumer is the `assert!` failure message does
  NOT trip `unused_variables` — it is genuinely read, just not where you
  meant (bitten 2026-08-09: `t0` was incremented correctly and still passed
  `0.0` into `integrate`). Same family as the bare-`two_body` bug: the
  compiler confirms the code is well-formed, never that it means what you
  intended. After wiring a new variable through, grep the call site.
- `cargo test` passing does NOT mean CI passes — clippy is a separate gate
  and CI is `-D warnings` (bitten 2026-08-09: `dced990` pushed green tests
  with a red `manual_range_contains`). Run
  `cargo clippy --workspace --all-targets -- -D warnings` before pushing,
  not just `cargo test`. Note `--all-targets` is what makes clippy look
  inside `#[cfg(test)]` modules at all; without it a lint in a test module
  is invisible locally and only appears in CI.
- `clippy::manual_range_contains`: a two-sided bound written
  `x >= lo && x <= hi` is a lint, not a style preference — CI rejects it.
  Write `(lo..=hi).contains(&x)`. Common in tolerance-band asserts.
- A test that PASSES prints nothing, so `--nocapture` will not show you a
  miss you want to record. To measure it, tighten the tolerance until the
  assert fails and read the number out of the panic message.
- **`is_nan()` where you meant `is_finite()` inverts a guard clause and
  compiles clean** (bitten 2026-08-10, `Burn::new`). `!accel.is_nan() ||
  accel < 0.0` rejects EVERY valid accel (2.943 is not NaN, so `!false` is
  true and the `||` short-circuits) while ACCEPTING NaN (`!true` is false,
  and `NaN < 0.0` is also false). Exactly backwards in both directions, and
  clippy says nothing — both are f64 → bool methods. Same family as the
  bare-`two_body` bug: well-formed, meaningless. `is_finite()` is also what
  catches ±infinity, which `is_nan()` never would. A validation function is
  precisely the code that looks obviously right and isn't — write the tests
  before trusting the read.
- Deriving `PartialEq` on an error enum whose variants carry f64 quietly
  breaks the NaN tests: `E::Invalid(f64::NAN) == E::Invalid(f64::NAN)` is
  FALSE. Don't derive it to make `assert_eq!` work; use
  `let Err(E::Invalid(v)) = … else { panic!() }` and assert `v.is_nan()`.
  `matches!` inside `assert!` also works but prints a useless message and
  discards the payload.
- **A symmetric round-trip assertion cannot catch a swapped getter.**
  `assert_eq!(a.accel(), b.accel())` passes even if `accel()` returns
  `self.duration`, because both sides return the same wrong field. Catching
  a field swap needs an ASYMMETRIC assertion against the literal passed to
  the constructor, with DISTINCT values per field (duration 60.0, accel
  100.0 — equal values can't catch a swap). Same family as the (M, e)
  gotcha above.
- **`assert!(result.is_err())` passes when your test data is malformed.**
  A JSON literal with a missing brace makes `from_str` return Err, so a
  validation test written as bare `is_err()` sits green while testing
  nothing (nearly shipped 2026-08-10 in `serde_rejection`). Assert on the
  message — `err.to_string().contains("must be finite")` — so the failure
  is provably YOURS and not a parse error. Generalizes: any test that can
  pass for the wrong reason isn't covering what its name claims.
- serde_json's "EOF while parsing an object" with a column equal to the
  LENGTH of your input means truncation (an unclosed `{`), not a typo
  mid-string. A malformed key or value points at a column in the middle.
- **hifitime `Epoch` serializes as a Display STRING, and the time-scale
  suffix is load-bearing.** `impl Serialize` is `serialize_str(&self.to_string())`,
  `Deserialize` is `Epoch::from_str`. So J2000 is
  `"2000-01-01T12:00:00 TT"` in JSON. Omitting ` TT` does NOT error — it
  parses as UTC, 64.184 s off, ~1,900 km of Earth motion, same hazard as
  `hifitime::J2000_REF_EPOCH`. Hand-written epoch JSON in tests and seed
  data must carry the scale.
- glam serializes `DVec3` as a 3-element ARRAY `[x, y, z]`, not an object
  with x/y/z keys. Verified for glam 0.33.2 with the `serde` feature.
- `Display` is NOT derivable — not for any type, ever. `Debug` is (for
  programmers); `Display` is user-facing and std won't guess. thiserror's
  `#[error("…")]` IS a Display-impl generator, which is why `BurnError` has
  one for free. Note `serde_json::to_string(&x)` is a free function going
  through `Serialize` and has nothing to do with `ToString`/`Display` —
  reaching for `.to_string()` on a struct is the wrong call and the
  "implement Display" error that follows is a red herring.
- Converting `Option` → `Result` is `ok_or(err)` / `ok_or_else(|| err)`,
  not an `is_none()` test — a boolean check throws away the value you then
  have to unwrap anyway. Use `ok_or` when the error is cheap to build (a
  Copy payload into an enum variant); `ok_or_else` only when construction
  allocates. Verified 2026-08-10: `ok_or_else(|| E::Variant(v))` DOES trip
  `clippy::unnecessary_lazy_evaluations` ("unnecessary closure used to
  substitute value for `Option::None`"), and CI is `-D warnings`, so the
  lazy form on a cheap error is a build failure. `is_none_or` is a DIFFERENT method
  (None, or Some matching a predicate) and is what autocomplete offers.
- `serde_json::from_str` is generic over its return type, so it needs an
  annotation — `let b: Burn = …` or `from_str::<Burn>(…)`. The turbofish
  reads better when there's no binding to annotate (e.g. asserting on an
  error).
- **An invariance/property test where BOTH sides land in the "nothing
  happened" state is a false pass** (bitten 2026-08-10,
  `reference_shift_invariance`). The first version used two references both
  AFTER the burn start; under an arg-flipped `seconds_since` both queries
  fell outside the (wrong) window, both returned `DVec3::ZERO`, they
  compared equal, and the test went green on broken code. Verified by
  simulating both versions against the exact test inputs. Fix was one
  reference straddling the burn (`*J2000 - 137.seconds()`), which makes the
  flipped version disagree. RULE: a property test must put at least one
  compared call in the INTERESTING state, or "equal" is trivially true.
- Exact `==` between a computed `length()` and the scalar it should equal
  is FIXTURE LUCK, not a property. `(2,-3,6).normalize() * a` has
  `length() == a` exactly (49 is a perfect square), but measured
  2026-08-10: `(0.11,-4.2,0.77777)` misses by 4.44e-16, `(1,1,1)` by
  4.44e-16, `(0.33,0.77,-0.1)*100` by 1.42e-14. `length()` is a sqrt of a
  sum of squares — a different arithmetic route from the scalar, so it
  needs a tolerance. Usually the whole-vector `distance(expected)` assert
  subsumes it anyway; prefer deleting the magnitude assert over adding a
  second tolerance.
- Normalizing an already-unit vector is not guaranteed to be the identity —
  the length of a normalized vector is `1.0 ± an ulp`, so dividing again
  can shift the last bit. A serde round-trip normalizes TWICE (once in
  `new`, once in `try_from`), so compare directions with a tolerance, never
  `==`. Scalars (f64 through serde_json) DO come back bit-identical.