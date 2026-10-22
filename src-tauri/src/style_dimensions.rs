//! Pure arithmetic for recovered STYLE_REPORT dimensions.
//!
//! Only Score and Decided Games currently have the required inputs. The other
//! dimensions remain unavailable rather than substituting descriptive proxies.
//! Constants below are independently implemented recovered facts, not copied
//! proprietary code. Native grade wrappers read an uninitialized prior output;
//! their categorical results are deliberately not reproduced.
use serde::{Deserialize, Serialize};

/// Actual results for one player's colour. `games` includes every processed
/// game, including unknown results. Wins contribute 1 point, draws 0.5 and all
/// other result codes 0 under the recovered native aggregation rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeResults {
    pub games: u64,
    pub wins: u64,
    pub draws: u64,
    pub unknown: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInput {
    pub label: String,
    pub numerator: Option<f64>,
    pub denominator: Option<u64>,
    /// An observed fraction, not a normalized dimension percentage. Missing
    /// colour data stays None even when native arithmetic substitutes rate 0.
    pub value: Option<f64>,
    pub reference: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeEvidence {
    pub ordinal: String,
    pub node: u32,
    pub ply: u32,
    pub san: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeDimension {
    pub id: String,
    pub label: String,
    pub description: String,
    pub raw_score: Option<f64>,
    /// Native plotting-range normalization, lower-clamped at 0. Composite
    /// values may exceed 100; the native graph applies its own separate cap.
    pub normalized: Option<f64>,
    pub normalization: [f64; 2],
    pub grade: Option<String>,
    pub grade_reason: String,
    pub status: String,
    pub unavailable_reason: Option<String>,
    pub inputs: Vec<NativeInput>,
    /// The pure scorer has no database access. The source-guarded aggregation
    /// layer may attach supporting positions after these calculations.
    pub evidence: Vec<NativeEvidence>,
}

const GRADE_REASON: &str = "Native categorical grades are unavailable: the recovered wrapper reads an uninitialized prior output.";

fn base(id: &str, label: &str, description: &str, bounds: [f64; 2]) -> NativeDimension {
    NativeDimension {
        id: id.into(),
        label: label.into(),
        description: description.into(),
        raw_score: None,
        normalized: None,
        normalization: bounds,
        grade: None,
        grade_reason: GRADE_REASON.into(),
        status: "unavailable".into(),
        unavailable_reason: None,
        inputs: vec![],
        evidence: vec![],
    }
}

fn input(label: &str, numerator: f64, games: u64, reference: Option<f64>) -> NativeInput {
    NativeInput {
        label: label.into(),
        numerator: Some(numerator),
        denominator: Some(games),
        value: (games > 0).then(|| numerator / games as f64),
        reference,
    }
}

fn calculated(white: NativeResults, black: NativeResults, draw_dimension: bool) -> NativeDimension {
    let mut result = if draw_dimension {
        base(
            "decidedGames",
            "Decided Games",
            "Native weighted draw-frequency comparison with the 0.543 reference for each colour; fewer draws contribute positively.",
            [-0.5, 1.0],
        )
    } else {
        base(
            "score",
            "Score",
            "Recorded points per processed game compared with the native 0.55 reference for each colour; White has weight 1.2 and Black 1.0.",
            [-1.5, 1.0],
        )
    };
    let (white_numerator, black_numerator, reference, white_label, black_label) = if draw_dimension
    {
        (
            white.draws as f64,
            black.draws as f64,
            0.543,
            "White draws",
            "Black draws",
        )
    } else {
        (
            white.wins as f64 + 0.5 * white.draws as f64,
            black.wins as f64 + 0.5 * black.draws as f64,
            0.55,
            "White points",
            "Black points",
        )
    };
    result.inputs = vec![
        input(white_label, white_numerator, white.games, Some(reference)),
        input(black_label, black_numerator, black.games, Some(reference)),
        input(
            "White unknown results",
            white.unknown as f64,
            white.games,
            None,
        ),
        input(
            "Black unknown results",
            black.unknown as f64,
            black.games,
            None,
        ),
    ];
    if white.games == 0 && black.games == 0 {
        result.status = "insufficient".into();
        result.unavailable_reason = Some("No processed games are available.".into());
        return result;
    }

    // Match the native zero-rate fallback for a missing colour, while keeping
    // its observed input None so absence is never displayed as measured zero.
    let white_rate = result.inputs[0].value.unwrap_or(0.0);
    let black_rate = result.inputs[1].value.unwrap_or(0.0);
    let raw = if draw_dimension {
        -1.5 * (white_rate / 0.543 - 1.0) - (black_rate / 0.543 - 1.0)
    } else {
        1.2 * (white_rate / 0.55 - 1.0) + (black_rate / 0.55 - 1.0)
    };
    result.raw_score = Some(raw);
    let [low, high] = result.normalization;
    result.normalized = Some(((raw - low) / (high - low) * 100.0).max(0.0));

    let mut caveats = vec![];
    if white.games == 0 {
        caveats.push("No White games: native arithmetic substitutes a zero White rate; the observed White input is unavailable.");
    }
    if black.games == 0 {
        caveats.push("No Black games: native arithmetic substitutes a zero Black rate; the observed Black input is unavailable.");
    }
    if white.unknown > 0 || black.unknown > 0 {
        caveats.push("The denominator includes unknown results, which contribute zero points and zero draws under the recovered native rule.");
    }
    if caveats.is_empty() {
        result.status = "available".into();
    } else {
        result.status = "partial".into();
        result.unavailable_reason = Some(caveats.join(" "));
    }
    result
}

fn unavailable(
    id: &str,
    label: &str,
    description: &str,
    bounds: [f64; 2],
    dependencies: &str,
    empty: bool,
) -> NativeDimension {
    let mut result = base(id, label, description, bounds);
    result.unavailable_reason = Some(if empty {
        result.status = "insufficient".into();
        format!("No processed games are available. {dependencies}")
    } else {
        dependencies.into()
    });
    result
}

/// The eight native dimensions in native order. This function only uses the
/// supplied tallies and never reads a database, launches an engine or fabricates
/// missing native observations. Sample guards belong to each recovered rule.
pub fn dimensions(white: NativeResults, black: NativeResults) -> Vec<NativeDimension> {
    let empty = white.games == 0 && black.games == 0;
    vec![
        calculated(white, black, false),
        unavailable(
            "theory",
            "Theory",
            "Native composite of matched opening depth, opening-classification diversity and selected-position evaluation averages.",
            [-2.5, 3.0],
            "Requires native opening-position repertoire classification from the Eco.cbcod/Eco.cod taxonomy or native service, plus selected-position evaluations. Stored header ECO does not supply these inputs.",
            empty,
        ),
        calculated(white, black, true),
        unavailable(
            "tenacity",
            "Tenacity",
            "Native composite of draw frequencies, short draws, mean draw length and position-transition observations.",
            [-2.0, 2.0],
            "The draw components are shown where available. A complete score also requires native exchange-opportunity counters and their exact phase/window rules; those terms are not omitted or replaced.",
            empty,
        ),
        unavailable(
            "aggressiveness",
            "Aggressiveness",
            "Native weighted thematic comparison plus king-safety, queen-retention and transition-related scalar observations.",
            [0.0, 6.0],
            "Requires native thematic property frequencies, king-safety coverage, queen-retention and transition scalars with their reference values.",
            empty,
        ),
        unavailable(
            "risk",
            "Risk",
            "Native weighted thematic comparison combined with king-safety and material-asymmetry observations.",
            [0.0, 2.0],
            "Requires native thematic property frequencies, king-safety coverage, asymmetric-material prevalence and their reference values.",
            empty,
        ),
        unavailable(
            "positionalPlay",
            "Positional Play",
            "Native weighted comparison of positional themes and king-safety observations.",
            [0.0, 7.0],
            "Requires the native positional-theme properties, eligibility and frequency records, king-safety coverage and their reference values.",
            empty,
        ),
        unavailable(
            "endgameAffinity",
            "Endgame Affinity",
            "Native composite of endgame frequency, performance differences and endgame point-rate subsets.",
            [0.0, 3.0],
            "Requires native endgame phase classification, rated performance subsets, equal/better/worse/technical endgame tallies, transition aggregates, reference values and sample guards.",
            empty,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: Option<f64>, expected: f64) {
        assert!((actual.expect("calculated value") - expected).abs() < 1e-10);
    }

    #[test]
    fn recovered_reference_rates_give_zero_raw_scores() {
        // (557 wins + 1086 half-points)/2000 = .55; 1086/2000 = .543.
        let results = NativeResults {
            games: 2000,
            wins: 557,
            draws: 1086,
            unknown: 0,
        };
        let all = dimensions(results, results);
        close(all[0].raw_score, 0.0);
        close(all[0].normalized, 60.0);
        close(all[2].raw_score, 0.0);
        close(all[2].normalized, 33.333333333333336);
        assert_eq!(all[0].status, "available");
        assert_eq!(all[2].status, "available");
    }

    #[test]
    fn actual_score_and_draw_ratios_use_distinct_recovered_colour_weights() {
        // White scores .7/draws .2; Black scores .4/draws .4.
        let all = dimensions(
            NativeResults {
                games: 10,
                wins: 6,
                draws: 2,
                unknown: 0,
            },
            NativeResults {
                games: 10,
                wins: 2,
                draws: 4,
                unknown: 0,
            },
        );
        close(all[0].raw_score, 0.05454545454545454);
        close(all[0].normalized, 62.18181818181818);
        close(all[2].raw_score, 1.210_865_561_694_291);
        close(all[2].normalized, 114.05770411295273);
        let white = &all[0].inputs[0];
        close(white.numerator, 7.0);
        assert_eq!(white.denominator, Some(10));
        close(white.value, 0.7);
        close(white.reference, 0.55);
    }

    #[test]
    fn empty_sample_is_insufficient_instead_of_a_calculated_native_zero_rate() {
        let all = dimensions(NativeResults::default(), NativeResults::default());
        for dimension in all {
            assert_eq!(dimension.status, "insufficient");
            assert!(dimension.raw_score.is_none());
            assert!(dimension.normalized.is_none());
            assert!(dimension.unavailable_reason.is_some());
        }
    }

    #[test]
    fn missing_colour_contributes_native_zero_but_is_disclosed_as_unobserved() {
        let all = dimensions(
            NativeResults::default(),
            NativeResults {
                games: 2,
                wins: 1,
                draws: 0,
                unknown: 0,
            },
        );
        close(all[0].raw_score, -1.290909090909091);
        close(all[0].normalized, 8.363636363636363);
        close(all[2].raw_score, 2.5);
        close(all[2].normalized, 200.0);
        for i in [0, 2] {
            assert_eq!(all[i].status, "partial");
            assert!(
                all[i]
                    .unavailable_reason
                    .as_ref()
                    .unwrap()
                    .contains("White")
            );
            assert_eq!(all[i].inputs[0].denominator, Some(0));
            assert!(all[i].inputs[0].value.is_none());
        }
    }

    #[test]
    fn unknown_results_stay_in_native_denominators_and_are_counted_explicitly() {
        let all = dimensions(
            NativeResults {
                games: 4,
                wins: 1,
                draws: 1,
                unknown: 1,
            },
            NativeResults {
                games: 4,
                wins: 2,
                draws: 0,
                unknown: 0,
            },
        );
        close(all[0].raw_score, -0.4727272727272727);
        close(all[0].normalized, 41.09090909090909);
        close(all[0].inputs[0].value, 0.375);
        assert_eq!(all[0].inputs[0].denominator, Some(4));
        for i in [0, 2] {
            assert_eq!(all[i].status, "partial");
            assert!(
                all[i]
                    .unavailable_reason
                    .as_ref()
                    .unwrap()
                    .contains("unknown")
            );
            assert!(
                all[i]
                    .inputs
                    .iter()
                    .any(|input| input.label.contains("White")
                        && input.label.contains("unknown")
                        && input.numerator == Some(1.0)
                        && input.denominator == Some(4)
                        && input.value == Some(0.25))
            );
        }
    }

    #[test]
    fn normalization_lower_clamps_without_upper_clamping_composite_scores() {
        let zero = NativeResults {
            games: 1,
            wins: 0,
            draws: 0,
            unknown: 0,
        };
        let all = dimensions(zero, zero);
        close(all[0].raw_score, -2.2);
        close(all[0].normalized, 0.0);
        close(all[2].normalized, 200.0);
        let win = NativeResults {
            games: 1,
            wins: 1,
            draws: 0,
            unknown: 0,
        };
        close(dimensions(win, win)[0].normalized, 132.0);
    }

    #[test]
    fn returns_eight_native_dimensions_in_order_with_unavailable_dependencies_and_no_grades() {
        let all = dimensions(
            NativeResults {
                games: 1,
                wins: 1,
                draws: 0,
                unknown: 0,
            },
            NativeResults {
                games: 1,
                wins: 0,
                draws: 1,
                unknown: 0,
            },
        );
        let expected = [
            ("score", "Score", [-1.5, 1.0]),
            ("theory", "Theory", [-2.5, 3.0]),
            ("decidedGames", "Decided Games", [-0.5, 1.0]),
            ("tenacity", "Tenacity", [-2.0, 2.0]),
            ("aggressiveness", "Aggressiveness", [0.0, 6.0]),
            ("risk", "Risk", [0.0, 2.0]),
            ("positionalPlay", "Positional Play", [0.0, 7.0]),
            ("endgameAffinity", "Endgame Affinity", [0.0, 3.0]),
        ];
        assert_eq!(all.len(), expected.len());
        for (dimension, (id, label, bounds)) in all.iter().zip(expected) {
            assert_eq!(dimension.id, id);
            assert_eq!(dimension.label, label);
            assert_eq!(dimension.normalization, bounds);
            assert!(dimension.grade.is_none());
            assert!(dimension.grade_reason.contains("uninitialized"));
            assert!(dimension.evidence.is_empty());
            if id != "score" && id != "decidedGames" {
                assert_eq!(dimension.status, "unavailable");
                assert!(dimension.raw_score.is_none());
                assert!(dimension.normalized.is_none());
                assert!(!dimension.unavailable_reason.as_ref().unwrap().is_empty());
            }
        }
        assert!(
            all[1]
                .unavailable_reason
                .as_ref()
                .unwrap()
                .contains("repertoire")
        );
        assert!(
            all[7]
                .unavailable_reason
                .as_ref()
                .unwrap()
                .contains("endgame")
        );
    }

    #[test]
    fn public_dtos_serialize_camel_case_without_inventing_grades_or_evidence() {
        let all = dimensions(NativeResults::default(), NativeResults::default());
        let value = serde_json::to_value(&all[0]).unwrap();
        assert!(value.get("rawScore").unwrap().is_null());
        assert!(value.get("gradeReason").unwrap().is_string());
        assert!(value.get("unavailableReason").unwrap().is_string());
        assert!(value.get("raw_score").is_none());
        assert_eq!(value["evidence"], serde_json::json!([]));
        let evidence = NativeEvidence {
            ordinal: "9007199254740993".into(),
            node: 7,
            ply: 5,
            san: "Qh5+".into(),
        };
        assert_eq!(
            serde_json::to_value(evidence).unwrap()["ordinal"],
            "9007199254740993"
        );
    }
}
