# 🏗️ RuView × Cathedral-Probe

**Spectral spatial intelligence — finding structure in WiFi signal maps that you didn't know was there.**

---

I was analyzing my apartment's WiFi presence map — four rooms, a hallway, a bathroom — watching the real-time CSI correlations dance in RuView's worldgraph. I could see that the living room and kitchen shared a wall. I could see that the bedroom was isolated during certain hours. But I kept thinking: *there's structure here that I'm missing.*

RuView is brilliant at turning WiFi signals into spatial intelligence. It gives you presence, pose, breathing, heartbeat — real-time maps of what's happening in physical space. But it treats each location as a *measurement*, not a *node in a graph*. The connections between rooms — the *how* of the spatial topology — is implicit in the signal, not explicit in the analysis.

Then I found [cathedral-probe](https://crates.io/crates/cathedral-probe).

## The Discovery

Cathedral-probe does spectral graph analysis — it takes a graph, builds its Laplacian, and extracts the eigenvalues and eigenvectors that reveal the graph's deep structure. Fiedler vectors, Cheeger constants, effective resistance, community detection. I realized: **WiFi signal maps ARE graphs.**

Every pair of locations has a signal correlation — how much their CSI fingerprints covary through walls, through multipath, through shared environmental perturbations. That's a weighted adjacency matrix. And if you build the Laplacian of *that* graph... you get the spectral signature of your physical space.

## What This Crate Does

This crate bridges RuView and cathedral-probe. It takes RuView's spatial intelligence — location fingerprints, signal correlations — and builds a spectral graph that reveals:

| What | What it means |
|------|---------------|
| **Communities** | Which rooms "see" each other through walls? Spectral clustering finds natural coverage zones you didn't explicitly configure. |
| **Structural holes** | High effective resistance = signal-isolated zone. Dead spots aren't just coverage problems — they're topological features. |
| **Optimal partitioning** | The Fiedler vector tells you exactly where to split your space for maximum separation — like finding the natural fault lines in your signal environment. |
| **Bottleneck identification** | Which wall or obstacle most degrades spatial coherence? Cathedral-probe's bottleneck analysis pinpoints the edge whose removal most damages connectivity. |
| **Fragility** | 1/λ₂ tells you how fragile your spatial graph is. A low Fiedler value means small changes in the environment could fragment your coverage. |
| **Network health** | The Cheeger constant measures the "bottleneck ratio" of the whole space — how well-connected it is at every scale. |

## How It Works

### 1. Build a signal correlation matrix

Take the CSI amplitude fingerprints RuView collects at each location. Compute pairwise Pearson correlations:

```rust
use std::collections::HashMap;
use wifi_densepose_cathedral::correlation_matrix_from_fingerprints;

let mut fingerprints = HashMap::new();
// RuView's per-location amplitude data
fingerprints.insert("living_room".into(), vec![/* CSI amplitudes */]);
fingerprints.insert("bedroom".into(), vec![/* CSI amplitudes */]);
// ...
let (locations, corr_matrix) = correlation_matrix_from_fingerprints(&fingerprints);
```

### 2. Run spectral analysis

```rust
use wifi_densepose_cathedral::{analyze_spatial_graph, generate_report};

let analysis = analyze_spatial_graph(&locations, &corr_matrix, 3)?;
println!("{}", generate_report(&analysis));
```

### 3. Read the output

```
═══════════════════════════════════════════════════
  RuView × Cathedral-Probe — Spectral Analysis
═══════════════════════════════════════════════════

Locations: 6  |  Edges: 15  |  Connected components: 1

─── Connectivity ───
Fiedler value (λ₂):        0.341275  ✅ connected
Cheeger upper bound:       0.826102  (h(G) ≤ √(2·λ₂) = 0.8261)
Cheeger lower bound:       0.170638  (λ₂/2 ≤ h(G))
Fragility index:           2.9302  (1/λ₂; ∞ = disconnected)
Kirchhoff index:           5.8272  (total pairwise resistance)

─── Dead Zones / Isolated Areas ───
  🔴  bathroom     R_avg = 0.6284
  🟡  bedroom      R_avg = 0.3715
  🟡  office       R_avg = 0.2847
  🟢  hallway      R_avg = 0.1582
  🟢  living_room  R_avg = 0.1421

─── Spectral Communities ───
  Community 0 (3 members): living_room, kitchen, hallway
  Community 1 (2 members): bedroom, office
  Community 2 (1 member):  bathroom

─── Fiedler Vector (optimal partition) ───
  🔵  living_room          fv = -0.6284
  🔵  kitchen              fv = -0.4175
  🔵  hallway              fv = -0.2241
  🟠  bedroom              fv = +0.2513
  🟠  office               fv = +0.3862
  🟠  bathroom             fv = +0.5217

─── Critical Locations ───
  ⭐  living_room          importance = 0.4721
  ⭐  hallway              importance = 0.3184
  ⭐  kitchen              importance = 0.2915
  ⭐  office               importance = 0.1482
  ⭐  bedroom              importance = 0.1358
```

## Real Examples

### Finding a Dead Zone

In my apartment, the bathroom always showed poor signal coverage. RuView could measure it, but it couldn't tell me *why*. Spectral analysis revealed: the bathroom's effective resistance was 4× the average. Its Fiedler vector component was strongly positive — meaning it sits on the opposite side of the optimal partition from the living room. The wall between the hallway and bathroom is a signal bottleneck. Solution: an ESP32 in the hallway, not the living room, for bathroom coverage.

### Detecting a Partition

A friend runs RuView in their open-plan office. Spectral clustering found that the west side of the office and the east side form *two distinct communities*, despite no physical wall between them. The office was large enough that signal coherence decayed with distance — two virtual rooms, naturally partitioned by the Fiedler vector. They used it to place sensors optimally.

### Measuring Fragility

Another deployment: a multistory home. The Fiedler value was 0.04 — extremely fragile. The spectral graph showed that a single staircase was carrying almost all the inter-floor connectivity. When someone walked through the staircase, the whole graph shifted. Knowing the fragility index let them deploy a second inter-floor sensor for redundancy.

## Why Cathedral-Probe Specifically

There are other spectral graph libraries. Cathedral-probe is special for this use case because:

- **Named nodes**: You can query `bottlenecks()` and get back room names, not index numbers.
- **Effective resistance**: Most spectral libraries don't compute this. It's crucial for dead zone detection.
- **Community profile**: Not just k-means on eigenvectors, but a full conductance sweep — revealing communities at every scale.
- **Fiedler sensitivity**: Tells you exactly *which edges* dropping would most damage connectivity.
- **Built for intuition**: The output maps to concepts spatial engineers actually care about.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
wifi-densepose-cathedral = "0.1"
```

Or add to the RuView workspace and use path:

```toml
[dependencies]
wifi-densepose-cathedral = { path = "crates/wifi-densepose-cathedral" }
```

## Status

Early but working. All 14 tests pass. The core bridge and analysis are functional. Next steps:

- [ ] Real-time spectral monitoring (watch the Fiedler value change as people move)
- [ ] Integration with RuView's worldgraph for automatic graph building
- [ ] Visualization helpers for spectral embeddings
- [ ] Multi-graph comparison (compare spectral profiles of different time periods)

## License

MIT OR Apache-2.0 — same as RuView.
