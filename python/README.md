# `wifi-densepose` v2.x — PyO3 bindings for the Rust core

This directory contains the source for the `wifi-densepose` PyPI wheel
(v2.0+). It's a PyO3 + maturin build that wraps the Rust crates in
[`v2/crates/`](../v2/crates/) and replaces the legacy pure-Python
`wifi-densepose==1.1.0` (released 2025-06-07).

See [ADR-117](../docs/adr/ADR-117-pip-wifi-densepose-modernization.md)
for the full modernization plan.

## Build locally

```bash
# Install maturin + dev deps
pip install maturin pytest

# Develop-install — builds the Rust extension in-place
cd python
maturin develop

# Run the smoke tests
pytest tests/
```

The `maturin develop` command produces a debug-build wheel installed
into your current Python environment. For release builds:

```bash
maturin build --release --strip
```

The wheel lands under `python/target/wheels/`.

## Layout

```
python/
├── Cargo.toml                    # PyO3 + abi3-py310 + Rust deps
├── pyproject.toml                # maturin backend + Python metadata
├── README.md                     # this file
├── src/
│   └── lib.rs                    # #[pymodule] — Rust binding glue
├── wifi_densepose/               # pure-Python facade (the user-facing API)
│   ├── __init__.py               # re-exports compiled module symbols
│   └── py.typed                  # PEP 561 typed-package marker
└── tests/
    └── test_smoke.py             # P1 acceptance tests
```

## Phase status (per ADR-117 §6)

- ✅ **P1 — Scaffold**: module loads, version constant exposed,
  6 smoke tests pass via `maturin develop`.
- ✅ **P2 — Core type bindings**: `Keypoint`, `KeypointType`,
  `BoundingBox`, `PersonPose`, `PoseEstimate`. 51 additional tests.
- ✅ **P3 — Vitals + signal DSP**: `VitalStatus`, `VitalEstimate`,
  `VitalReading`, `BreathingExtractor`, `HeartRateExtractor` with
  `py.allow_threads` GIL release on hot loops (Q5 tokio audit on
  2026-05-24 confirmed core/vitals/signal are pure-sync). 17 tests.
- ✅ **P3.5 — BFLD bindings (stub Rust)**: `BfldKind`, `BfldFrame`,
  `BfldReport` — forward-compatible Python surface for 802.11ac/ax/be
  Beamforming Feedback Loop Data. numpy Complex64 bridge. 19 tests.
  Real Rust ingestion lands post-v2.0 in a `wifi-densepose-bfld`
  crate (see ADR-117 §11.11/12); the Python API does not change.
- ✅ **P4 — WS/MQTT client**: pure-Python `wifi_densepose.client` extra
  (no Rust). `SensingClient` (asyncio websockets), `RuViewMqttClient`
  (paho-mqtt v2 with VERSION2 callbacks), `HABlueprintHelper` (HA
  discovery payload parser), `SemanticPrimitiveListener` (typed router
  for the 10 HA-MIND primitives from ADR-115 §3.12). 63 tests including
  end-to-end against an in-process `websockets.serve` fixture.
- ⏳ **P5 — cibuildwheel + PyPI publish**: Linux/macOS/Windows × abi3-py310.
- ⏳ **P-tomb — v1.99.0 tombstone wheel**: pure-Python ImportError
  with migration URL, published to PyPI to soft-fence v1.x users
  before v2.0 ships.

Each phase ends with a checkbox PR. Tests are additive — every phase's
smoke tests must still pass after later phases land.

## Migrating from v1.x

The v1 line was a separate pure-Python implementation. v2 is a hard
break (semver-justified by 11.5 months of stack drift). Migration
guide ships in [docs/migrations/wifi-densepose-1-to-2.md](../docs/migrations/wifi-densepose-1-to-2.md)
(landing in P5).
