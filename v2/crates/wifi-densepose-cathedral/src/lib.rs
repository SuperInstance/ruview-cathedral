//! # RuView × Cathedral-Probe: Spectral Spatial Intelligence
//!
//! This crate bridges RuView's WiFi spatial intelligence with
//! [`cathedral-probe`](https://crates.io/crates/cathedral-probe) spectral
//! graph analysis. It turns RuView's raw CSI signal correlations and
//! room-level presence data into a weighted graph, then applies spectral
//! methods — community detection, Fiedler partitioning, Cheeger constants,
//! effective resistance — to reveal structural patterns that raw signal
//! processing misses.
//!
//! ## Why this exists
//!
//! RuView maps physical space from WiFi signals. Every room, every sensor
//! node, every person-track is a data point in a spatial graph. But RuView
//! processes signals, not graphs — it tells you *where* things are and *how
//! they move*, but not *how the space is structured*.
//!
//! That's where cathedral-probe comes in. By building an adjacency graph
//! from signal correlations between pairs of locations, we can:
//!
//! - **Find communities**: Which rooms "see" each other through walls?
//!   Which zones form natural clusters of WiFi coverage?
//! - **Detect structural holes**: Where are the dead zones? Which areas
//!   are signal-isolated from the rest of the space?
//! - **Measure connectivity**: How healthy is the overall spatial graph?
//!   A low Fiedler value means fragile coverage — a high one means
//!   robust multi-path richness.
//! - **Partition optimally**: Use the Fiedler vector to split the space
//!   into optimally-separated sensing zones, each with its own coverage
//!   characteristics.
//! - **Identify bottlenecks**: Which walls or obstacles most degrade
//!   spatial coherence? The `fiedler_sensitivity` tells us exactly which
//!   edges matter most.

// Re-export cathedral-probe types so callers don't need a direct dep.
pub use cathedral_probe::{
    CathedralProbe, SparseCathedralProbe, DirectedCathedralProbe, SpectrumResult,
    SpectrumMethod, ComponentAnalysis, CathedralError,
};

use std::collections::HashMap;

// =============================================================================
// Spatial graph types
// =============================================================================

/// A named location in the spatial graph — a room, zone, or sensor coverage area.
pub type LocationId = String;

/// Signal correlation between two locations, in the range [0.0, 1.0].
///
/// 1.0 = locations see the same WiFi perturbations (likely same or adjacent room).
/// 0.0 = completely independent signal environments.
pub type SignalCorrelation = f64;

/// A weighted edge in the spatial signal graph.
#[derive(Debug, Clone)]
pub struct SpatialEdge {
    /// Source location name.
    pub from: LocationId,
    /// Target location name.
    pub to: LocationId,
    /// Signal correlation strength [0, 1].
    pub correlation: SignalCorrelation,
}

// =============================================================================
// RuView → Cathedral bridge
// =============================================================================

/// Build a `cathedral_probe::CathedralProbe` from RuView location data and
/// signal correlations.
///
/// This is the primary integration point. You feed it:
/// - your named locations (rooms, zones, sensor positions)
/// - the signal correlations between each pair
///
/// And you get back a fully-instrumented spectral graph ready for analysis.
/// Node indices in results correspond to the order of `locations`.
pub fn build_spatial_graph(
    locations: &[LocationId],
    correlation_matrix: &[Vec<f64>],
) -> Result<CathedralProbe, CathedralError> {
    // `from_matrix` assigns node IDs "0", "1", "2", ... which map directly
    // to our location indices. We build a named graph for query methods that
    // accept &str (bottlenecks, component_importance, etc.)
    let names: Vec<&str> = locations.iter().map(|s| s.as_str()).collect();
    let mut g = CathedralProbe::new(names);
    for i in 0..correlation_matrix.len() {
        for j in (i + 1)..correlation_matrix.len() {
            let w = correlation_matrix[i][j];
            if w > 0.0 {
                g.connect(&locations[i], &locations[j], w);
            }
        }
    }
    Ok(g)
}

/// Build a sparse spatial graph for large deployments (100+ nodes).
pub fn build_sparse_spatial_graph(
    locations: &[LocationId],
    edges: &[(usize, usize, f64)],
) -> SparseCathedralProbe {
    let names: Vec<String> = locations.to_vec();
    SparseCathedralProbe::from_edges(locations.len(), edges)
        .with_names(names)
}

// =============================================================================
// High-level analysis
// =============================================================================

/// Full spectral analysis of a WiFi spatial graph.
#[derive(Debug, Clone)]
pub struct SpatialSpectralAnalysis {
    /// Locations in the graph.
    pub locations: Vec<LocationId>,
    /// Room-to-room correlation matrix.
    pub correlations: Vec<Vec<SignalCorrelation>>,

    // ─── Spectral metrics ───────────────────────────────────
    /// Fiedler value (algebraic connectivity) of the full graph.
    pub fiedler_value: f64,
    /// Cheeger upper bound: √(2·λ₂) — upper bound on the isoperimetric number.
    pub cheeger_upper: f64,
    /// Cheeger lower bound: λ₂/2 — lower bound on the isoperimetric number.
    pub cheeger_lower: f64,
    /// Condition number of the Laplacian: λ_max / λ_min_nonzero.
    pub condition_number: f64,
    /// Number of connected components (1 = fully connected).
    pub connected_components: usize,
    /// Is the graph fully connected?
    pub is_fully_connected: bool,

    // ─── Per-location analysis ──────────────────────────────
    /// Effective resistance from each location to all others (averaged).
    /// High values = isolated (hard for signals to propagate).
    pub effective_resistance_avg: Vec<(LocationId, f64)>,

    /// The Fiedler vector (second eigenvector of the Laplacian).
    /// Used to partition the space optimally.
    pub fiedler_vector: Vec<(LocationId, f64)>,

    /// Community/cluster assignment from spectral clustering.
    pub communities: Vec<(LocationId, usize)>,

    /// Kirchhoff index — total pairwise effective resistance.
    /// Lower = more robust signal environment.
    pub kirchhoff_index: f64,

    /// Component importance — which locations are most crucial for connectivity.
    pub location_importance: Vec<(LocationId, f64)>,

    /// Bottleneck edges — which location pairs most degrade connectivity.
    pub bottleneck_pairs: Vec<(LocationId, LocationId, f64)>,

    /// Fragility index: 1 / Fiedler. Infinity = disconnected.
    pub fragility_index: f64,

    /// Per-component analysis (useful when graph is disconnected).
    pub per_component: Vec<ComponentAnalysis>,

    /// Full Laplacian spectrum (all eigenvalues).
    pub spectrum: Vec<f64>,

    /// Community profile: at each partition size s, the minimum conductance Φ(s).
    pub community_profile: Vec<(usize, f64)>,
}

/// Compute a full spectral analysis on a spatial graph built from correlations.
///
/// # Arguments
///
/// * `locations` — Named locations (rooms, zones, sensor nodes).
/// * `correlations` — n×n correlation matrix where `correlations[i][j]` is the
///   signal correlation between location `i` and location `j`.
/// * `num_clusters` — Desired number of spectral communities.
///
/// # Returns
///
/// A `SpatialSpectralAnalysis` with all computed metrics.
pub fn analyze_spatial_graph(
    locations: &[LocationId],
    correlations: &[Vec<f64>],
    num_clusters: usize,
) -> Result<SpatialSpectralAnalysis, CathedralError> {
    let g = build_spatial_graph(locations, correlations)?;
    let n = g.node_count();

    let spectrum = g.spectrum_result();
    let eigenvalues = spectrum.eigenvalues.clone();
    let fiedler_value = g.fiedler_value();
    let cheeger_upper = g.cheeger_upper_bound();
    let cheeger_lower = g.cheeger_lower_bound();
    let condition_number = g.condition_number();
    let cc = g.connected_components();
    let is_connected = g.is_connected();
    let fragility = g.fragility_index();
    let component_analysis = g.per_component_analysis();
    let community_profile = g.community_profile();
    let kirchhoff = g.kirchhoff_index();

    // --- Fiedler vector ---
    // Use spectral_embedding(1) to get the Fiedler vector (1-dim embedding = 2nd eigenvector).
    // The embedding maps each node to its coordinate along the 2nd-smallest eigenvector.
    let embed = g.spectral_embedding(1);
    let fiedler_vec: Vec<(LocationId, f64)> = if embed.len() == n && n > 1 {
        locations.iter().enumerate()
            .map(|(i, loc)| (loc.clone(), embed[i].first().copied().unwrap_or(0.0)))
            .collect()
    } else {
        locations.iter().map(|loc| (loc.clone(), 0.0)).collect()
    };

    // --- Effective resistance (average per location) ---
    let eff_res_avg: Vec<(LocationId, f64)> = if n >= 2 {
        locations.iter().enumerate()
            .map(|(i, loc)| {
                let avg_r: f64 = (0..n)
                    .filter(|&j| j != i)
                    .map(|j| g.effective_resistance(i, j))
                    .sum::<f64>()
                    / (n - 1) as f64;
                (loc.clone(), avg_r)
            })
            .collect()
    } else {
        locations.iter().map(|loc| (loc.clone(), 0.0)).collect()
    };

    // --- Spectral clustering ---
    let k = num_clusters.max(1).min(n);
    let cluster_assignments = g.spectral_cluster(k);
    let communities: Vec<(LocationId, usize)> = locations.iter()
        .enumerate()
        .map(|(i, loc)| (loc.clone(), cluster_assignments.get(i).copied().unwrap_or(0)))
        .collect();

    // --- Component importance ---
    let importance_map = g.component_importance();
    let mut location_importance: Vec<(LocationId, f64)> = locations.iter()
        .map(|loc| (loc.clone(), importance_map.get(loc).copied().unwrap_or(0.0)))
        .collect();
    location_importance.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // --- Bottleneck pairs ---
    let bn = g.bottlenecks();
    let bottleneck_pairs: Vec<(LocationId, LocationId, f64)> = bn.into_iter()
        .map(|(a, b, drop)| {
            // a and b are the original node names (since our named graph uses them)
            (a, b, drop)
        })
        .collect();

    Ok(SpatialSpectralAnalysis {
        locations: locations.to_vec(),
        correlations: correlations.to_vec(),
        fiedler_value,
        cheeger_upper,
        cheeger_lower,
        condition_number,
        connected_components: cc,
        is_fully_connected: is_connected,
        effective_resistance_avg: eff_res_avg,
        fiedler_vector: fiedler_vec,
        communities,
        kirchhoff_index: kirchhoff,
        location_importance,
        bottleneck_pairs,
        fragility_index: fragility,
        per_component: component_analysis,
        spectrum: eigenvalues,
        community_profile,
    })
}

// =============================================================================
// Convenience builders for RuView data
// =============================================================================

/// Build a signal correlation matrix from RuView's per-location CSI magnitude
/// fingerprints.
///
/// Computes the Pearson correlation between each pair of locations'
/// amplitude time-series. This is the most natural way to turn RuView's
/// spatial intelligence data into a graph — if two rooms see correlated
/// signal variations, they're connected through walls or shared paths.
///
/// # Arguments
///
/// * `location_fingerprints` — Map from location name to a 1-D array of
///   amplitude samples (e.g., mean amplitude over time at each subcarrier).
pub fn correlation_matrix_from_fingerprints(
    location_fingerprints: &HashMap<LocationId, Vec<f64>>,
) -> (Vec<LocationId>, Vec<Vec<f64>>) {
    let locations: Vec<LocationId> = location_fingerprints.keys().cloned().collect();
    let n = locations.len();
    let mut matrix = vec![vec![0.0f64; n]; n];

    for i in 0..n {
        for j in i..n {
            let a = &location_fingerprints[&locations[i]];
            let b = &location_fingerprints[&locations[j]];
            let corr = pearson_correlation(a, b);
            matrix[i][j] = corr;
            matrix[j][i] = corr;
        }
    }

    (locations, matrix)
}

/// Pearson correlation coefficient between two samples.
fn pearson_correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }

    let mean_a: f64 = a.iter().sum::<f64>() / n as f64;
    let mean_b: f64 = b.iter().sum::<f64>() / n as f64;

    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;

    for i in 0..n {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    let denom = (var_a * var_b).sqrt();
    if denom < 1e-15 {
        0.0
    } else {
        cov / denom
    }
}

// =============================================================================
// Report generation
// =============================================================================

/// Generate a human-readable report from a `SpatialSpectralAnalysis`.
pub fn generate_report(analysis: &SpatialSpectralAnalysis) -> String {
    let mut report = String::new();
    report.push_str("═══════════════════════════════════════════════════\n");
    report.push_str("  RuView × Cathedral-Probe — Spectral Analysis\n");
    report.push_str("═══════════════════════════════════════════════════\n\n");

    // Overview
    report.push_str(&format!(
        "Locations: {}  |  Edges: {}  |  Connected components: {}\n\n",
        analysis.locations.len(),
        analysis.locations.len() * (analysis.locations.len() - 1) / 2,
        analysis.connected_components,
    ));

    // Connectivity
    report.push_str("─── Connectivity ───\n");
    report.push_str(&format!(
        "Fiedler value (λ₂):        {:.6}  {}\n",
        analysis.fiedler_value,
        if analysis.is_fully_connected { "✅ connected" } else { "⚠️  disconnected" }
    ));
    report.push_str(&format!(
        "Cheeger upper bound:       {:.6}  (h(G) ≤ √(2·λ₂) = {:.4})\n",
        analysis.cheeger_upper,
        analysis.cheeger_upper,
    ));
    report.push_str(&format!(
        "Cheeger lower bound:       {:.6}  (λ₂/2 ≤ h(G))\n",
        analysis.cheeger_lower,
    ));
    report.push_str(&format!(
        "Fragility index:           {:.4}  (1/λ₂; ∞ = disconnected)\n",
        analysis.fragility_index,
    ));
    report.push_str(&format!(
        "Condition number:          {:.4}\n",
        analysis.condition_number,
    ));
    report.push_str(&format!(
        "Kirchhoff index:           {:.4}  (total pairwise resistance)\n\n",
        analysis.kirchhoff_index,
    ));

    // Dead zones (high effective resistance)
    report.push_str("─── Dead Zones / Isolated Areas ───\n");
    let mut sorted_r = analysis.effective_resistance_avg.clone();
    sorted_r.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (loc, r) in sorted_r.iter().take(5) {
        let icon = if *r > 0.5 { "🔴" } else if *r > 0.2 { "🟡" } else { "🟢" };
        report.push_str(&format!(
            "  {}  {}  R_avg = {:.4}\n",
            icon, loc, r,
        ));
    }
    report.push_str("\n");

    // Communities
    report.push_str("─── Spectral Communities ───\n");
    let mut community_map: HashMap<usize, Vec<String>> = HashMap::new();
    for (loc, c) in &analysis.communities {
        community_map.entry(*c).or_default().push(loc.clone());
    }
    let mut community_ids: Vec<usize> = community_map.keys().copied().collect();
    community_ids.sort_unstable();
    for cid in community_ids {
        if let Some(members) = community_map.get(&cid) {
            report.push_str(&format!(
                "  Community {} ({} members): {}\n",
                cid,
                members.len(),
                members.join(", "),
            ));
        }
    }
    report.push_str("\n");

    // Fiedler vector
    report.push_str("─── Fiedler Vector (optimal partition) ───\n");
    let mut sorted_fv = analysis.fiedler_vector.clone();
    sorted_fv.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    for (loc, val) in &sorted_fv {
        let side = if *val < 0.0 { "🔵" } else { "🟠" };
        report.push_str(&format!("  {}  {:20}  fv = {:+.4}\n", side, loc, val));
    }
    report.push_str("\n");

    // Bottleneck edges
    if !analysis.bottleneck_pairs.is_empty() {
        report.push_str("─── Bottleneck Edges ───\n");
        for (a, b, drop) in analysis.bottleneck_pairs.iter().take(5) {
            report.push_str(&format!(
                "  ⚡  {} ↔ {}   Fiedler drop = {:.4}\n",
                a, b, drop,
            ));
        }
        report.push_str("\n");
    }

    // Component importance
    if !analysis.location_importance.is_empty() {
        report.push_str("─── Critical Locations ───\n");
        for (loc, imp) in analysis.location_importance.iter().take(5) {
            report.push_str(&format!(
                "  ⭐  {:20}  importance = {:.4}\n",
                loc, imp,
            ));
        }
        report.push_str("\n");
    }

    // Community profile summary
    if !analysis.community_profile.is_empty() {
        let min_cond = analysis.community_profile.iter()
            .map(|(_, c)| *c)
            .fold(f64::MAX, f64::min);
        report.push_str(&format!(
            "─── Community Profile ───\n  Best conductance: Φ = {:.4}\n\n",
            min_cond,
        ));
    }

    report.push_str("═══════════════════════════════════════════════════\n");
    report
}

// =============================================================================
// Quick demo / example
// =============================================================================

/// Run a quick spectral analysis on a demo spatial graph, printing results.
///
/// This is the concrete example from the README — analyzing a 4-room apartment
/// with signal correlations.
pub fn run_living_space_demo() {
    let locations = vec![
        "living_room".to_string(),
        "bedroom".to_string(),
        "kitchen".to_string(),
        "office".to_string(),
        "hallway".to_string(),
        "bathroom".to_string(),
    ];

    // Simulated signal correlations between rooms.
    // These represent how much each pair of locations "sees" correlated WiFi
    // perturbations — high = through-wall proximity, low = independent zones.
    let correlations = vec![
        // living  bedroom  kitchen  office   hallway  bathroom
        vec![1.00,   0.65,    0.80,    0.45,    0.70,    0.30],  // living_room
        vec![0.65,   1.00,    0.40,    0.75,    0.60,    0.25],  // bedroom
        vec![0.80,   0.40,    1.00,    0.30,    0.55,    0.50],  // kitchen
        vec![0.45,   0.75,    0.30,    1.00,    0.50,    0.20],  // office
        vec![0.70,   0.60,    0.55,    0.50,    1.00,    0.60],  // hallway
        vec![0.30,   0.25,    0.50,    0.20,    0.60,    1.00],  // bathroom
    ];

    match analyze_spatial_graph(&locations, &correlations, 3) {
        Ok(analysis) => {
            println!("{}", generate_report(&analysis));
        }
        Err(e) => {
            eprintln!("Analysis failed: {e}");
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correlation_matrix() {
        let mut fingerprints = HashMap::new();
        fingerprints.insert("room_a".into(), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        fingerprints.insert("room_b".into(), vec![1.0, 2.0, 4.0, 4.0, 5.0]);
        fingerprints.insert("room_c".into(), vec![5.0, 4.0, 3.0, 2.0, 1.0]);

        let (locs, mat) = correlation_matrix_from_fingerprints(&fingerprints);
        assert_eq!(locs.len(), 3);
        assert_eq!(mat.len(), 3);
        assert!((mat[0][0] - 1.0).abs() < 0.01);
        // room_a and room_b are similar, so correlation should be positive
        assert!(mat[0][1] > 0.8);
        // room_a and room_c are anti-correlated
        assert!(mat[0][2] < -0.8);
    }

    #[test]
    fn test_pearson_correlation() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let c = vec![5.0, 4.0, 3.0, 2.0, 1.0];

        assert!((pearson_correlation(&a, &b) - 1.0).abs() < 0.01);
        assert!((pearson_correlation(&a, &c) - (-1.0)).abs() < 0.01);
        assert!((pearson_correlation(&a, &a) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_pearson_constant() {
        let a = vec![3.0, 3.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        // Constant has zero variance, correlation returns 0
        assert!((pearson_correlation(&a, &b) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_build_spatial_graph() {
        let locations = vec!["a".into(), "b".into(), "c".into()];
        let correlations = vec![
            vec![1.0, 0.5, 0.2],
            vec![0.5, 1.0, 0.8],
            vec![0.2, 0.8, 1.0],
        ];

        let g = build_spatial_graph(&locations, &correlations).unwrap();
        assert_eq!(g.node_count(), 3);
    }

    #[test]
    fn test_build_sparse_graph() {
        let locations = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let edges = vec![(0, 1, 0.8), (1, 2, 0.6), (2, 3, 0.7)];

        let sparse = build_sparse_spatial_graph(&locations, &edges);
        assert_eq!(sparse.node_count(), 4);
    }

    #[test]
    fn test_analyze_living_space() {
        let locations = vec![
            "living_room".into(), "bedroom".into(), "kitchen".into()
        ];
        let correlations = vec![
            vec![1.0, 0.6, 0.8],
            vec![0.6, 1.0, 0.4],
            vec![0.8, 0.4, 1.0],
        ];

        let analysis = analyze_spatial_graph(&locations, &correlations, 2).unwrap();
        assert!(analysis.fiedler_value > 0.0);
        assert_eq!(analysis.locations.len(), 3);
        assert_eq!(analysis.communities.len(), 3);
        assert!(analysis.kirchhoff_index > 0.0);

        // Fiedler vector should have 3 entries
        assert_eq!(analysis.fiedler_vector.len(), 3);

        // Location importance
        assert_eq!(analysis.location_importance.len(), 3);

        // Report should not explode
        let report = generate_report(&analysis);
        assert!(report.contains("Fiedler value"));
        assert!(report.contains("living_room"));
    }

    #[test]
    fn test_demo_runs() {
        // Just verify it doesn't panic
        run_living_space_demo();
    }

    #[test]
    fn test_single_location() {
        let locations = vec!["only_room".into()];
        let correlations = vec![vec![1.0]];
        let analysis = analyze_spatial_graph(&locations, &correlations, 1).unwrap();
        assert_eq!(analysis.fiedler_value, 0.0);
        assert_eq!(analysis.spectrum.len(), 1);
    }

    #[test]
    fn test_two_locations_connected() {
        let locations = vec!["room_a".into(), "room_b".into()];
        let correlations = vec![
            vec![1.0, 0.9],
            vec![0.9, 1.0],
        ];
        let analysis = analyze_spatial_graph(&locations, &correlations, 1).unwrap();
        assert!(analysis.fiedler_value > 0.0);
        assert!(analysis.is_fully_connected);
    }

    #[test]
    fn test_disconnected_locations() {
        let locations = vec!["room_a".into(), "room_b".into()];
        let correlations = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
        ];
        let analysis = analyze_spatial_graph(&locations, &correlations, 1).unwrap();
        // Two disconnected nodes → Fiedler = 0
        assert!(analysis.fiedler_value < 1e-10);
        assert!(!analysis.is_fully_connected);
    }

    #[test]
    fn test_effective_resistance_meaningful() {
        let locations = vec!["center".into(), "near".into(), "far".into()];
        // Central node strongly connected to near, weakly to far
        let correlations = vec![
            vec![1.0, 0.9, 0.1],
            vec![0.9, 1.0, 0.1],
            vec![0.1, 0.1, 1.0],
        ];
        let analysis = analyze_spatial_graph(&locations, &correlations, 2).unwrap();

        // The "far" node should have higher effective resistance (more isolated)
        let far_r = analysis.effective_resistance_avg.iter()
            .find(|(loc, _)| loc == "far")
            .map(|(_, r)| *r)
            .unwrap_or(0.0);
        let center_r = analysis.effective_resistance_avg.iter()
            .find(|(loc, _)| loc == "center")
            .map(|(_, r)| *r)
            .unwrap_or(0.0);
        assert!(
            far_r > center_r,
            "Isolated 'far' should have higher effective resistance ({far_r}) than 'center' ({center_r})"
        );
    }

    #[test]
    fn test_generate_report_format() {
        let locations = vec!["a".into(), "b".into(), "c".into()];
        let correlations = vec![
            vec![1.0, 0.5, 0.3],
            vec![0.5, 1.0, 0.7],
            vec![0.3, 0.7, 1.0],
        ];
        let analysis = analyze_spatial_graph(&locations, &correlations, 2).unwrap();
        let report = generate_report(&analysis);
        assert!(report.starts_with("══"));
        assert!(report.contains("RuView × Cathedral-Probe"));
        assert!(report.contains("Connectivity"));
        assert!(report.contains("Dead Zones"));
        assert!(report.contains("Spectral Communities"));
        assert!(report.contains("Fiedler Vector"));
    }

    #[test]
    fn test_large_correlation_matrix() {
        // 50 locations with random correlations — just verify no crashes
        let locations: Vec<String> = (0..50).map(|i| format!("zone_{i}")).collect();
        let mut correlations = vec![vec![0.0; 50]; 50];
        for i in 0..50 {
            correlations[i][i] = 1.0;
            for j in (i + 1)..50 {
                let c = 0.5 + 0.5 * ((i as f64 * j as f64).sin());
                correlations[i][j] = c;
                correlations[j][i] = c;
            }
        }
        let analysis = analyze_spatial_graph(&locations, &correlations, 5).unwrap();
        assert_eq!(analysis.locations.len(), 50);
        assert!(analysis.fiedler_value > 0.0);
        assert_eq!(analysis.connected_components, 1);
    }

    #[test]
    fn test_fiedler_partition_symmetry() {
        // Two clusters: A={0,1,2} and B={3,4,5} with weak bridge
        let locations: Vec<String> = (0..6).map(|i| format!("n{i}")).collect();
        let mut correlations = vec![vec![0.0; 6]; 6];
        for i in 0..6 {
            correlations[i][i] = 1.0;
        }
        // Cluster A (0,1,2) strong internal
        for &i in &[0, 1, 2] {
            for &j in &[0, 1, 2] {
                if i < j {
                    correlations[i][j] = 0.9;
                    correlations[j][i] = 0.9;
                }
            }
        }
        // Cluster B (3,4,5) strong internal
        for &i in &[3, 4, 5] {
            for &j in &[3, 4, 5] {
                if i < j {
                    correlations[i][j] = 0.9;
                    correlations[j][i] = 0.9;
                }
            }
        }
        // Weak bridge between clusters
        correlations[2][3] = 0.1;
        correlations[3][2] = 0.1;

        let analysis = analyze_spatial_graph(&locations, &correlations, 2).unwrap();
        // Fiedler vector should separate the two clusters (roughly half positive, half negative)
        let fiedler_vals: Vec<f64> = analysis.fiedler_vector.iter().map(|(_, v)| *v).collect();
        let cluster_a_sign = fiedler_vals[0].signum();
        let cluster_b_sign = fiedler_vals[3].signum();
        assert_ne!(
            cluster_a_sign,
            cluster_b_sign,
            "Fiedler vector should separate clusters: A={cluster_a_sign}, B={cluster_b_sign}"
        );
    }
}
