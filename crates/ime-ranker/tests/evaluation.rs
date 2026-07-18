use std::time::Instant;

use radishlex_ime_core::Candidate;
use radishlex_ime_ranker::{DeletedTermSummary, RankRequest, Ranker};
use radishlex_ime_userdb::{RankerWeight, TermSource, TermStatus, UserTerm};

const EVALUATED_AT_MS: i64 = 2_000_000_000_000;

#[derive(Debug)]
struct EvaluationMetrics {
    cases: usize,
    top_1: f64,
    top_3: f64,
    mrr: f64,
}

#[test]
fn fixed_synthetic_ranking_set_matches_quality_baseline() {
    let ranker = Ranker::default();
    let cases = synthetic_cases();
    let mut top_1_hits = 0_usize;
    let mut top_3_hits = 0_usize;
    let mut reciprocal_rank = 0.0;

    for case in &cases {
        let ranked = ranker
            .rank(case.request.clone())
            .expect("synthetic case ranks");
        let rank = ranked
            .iter()
            .position(|candidate| candidate.candidate.text() == case.expected_text)
            .expect("expected candidate remains present")
            + 1;
        top_1_hits += usize::from(rank == 1);
        top_3_hits += usize::from(rank <= 3);
        reciprocal_rank += 1.0 / rank as f64;
    }

    let metrics = EvaluationMetrics {
        cases: cases.len(),
        top_1: top_1_hits as f64 / cases.len() as f64,
        top_3: top_3_hits as f64 / cases.len() as f64,
        mrr: reciprocal_rank / cases.len() as f64,
    };
    eprintln!("synthetic ranking baseline: {metrics:?}");

    assert_eq!(metrics.cases, 5);
    assert_eq!(metrics.top_1, 0.8);
    assert_eq!(metrics.top_3, 1.0);
    assert_eq!(metrics.mrr, 0.9);
}

#[test]
fn fixed_candidate_rerank_latency_baseline_is_finite_without_ci_wall_clock_gate() {
    let ranker = Ranker::default();
    let candidates = (0..50)
        .map(|index| Candidate::new(format!("合成候选{index:02}")))
        .collect::<Vec<_>>();
    let weights = (0..10)
        .map(|index| RankerWeight {
            input_code: "latency".to_owned(),
            text: format!("合成候选{index:02}"),
            reading: None,
            frequency: i64::from(index + 1),
            last_used_at_ms: Some(EVALUATED_AT_MS - i64::from(index) * 1_000),
            negative_score: 0.0,
            context_kind: "general".to_owned(),
        })
        .collect::<Vec<_>>();
    let request =
        RankRequest::new("latency", candidates, EVALUATED_AT_MS).with_ranker_weights(weights);

    for _ in 0..100 {
        std::hint::black_box(
            ranker
                .rank(request.clone())
                .expect("warm-up rerank succeeds"),
        );
    }
    let iterations = 1_000_u32;
    let started = Instant::now();
    for _ in 0..iterations {
        let ranked = ranker
            .rank(request.clone())
            .expect("measured rerank succeeds");
        assert_eq!(ranked.len(), 50);
        std::hint::black_box(ranked);
    }
    let elapsed = started.elapsed();
    let micros_per_iteration = elapsed.as_secs_f64() * 1_000_000.0 / f64::from(iterations);
    eprintln!(
        "synthetic rerank latency baseline: candidates=50 iterations={iterations} total_us={} us_per_iteration={micros_per_iteration:.3}",
        elapsed.as_micros()
    );

    assert!(elapsed.as_nanos() > 0);
    assert!(micros_per_iteration.is_finite());
    assert!(micros_per_iteration > 0.0);
}

#[derive(Debug)]
struct SyntheticCase {
    expected_text: &'static str,
    request: RankRequest,
}

fn synthetic_cases() -> Vec<SyntheticCase> {
    vec![
        SyntheticCase {
            expected_text: "合成甲二",
            request: RankRequest::new(
                "case-a",
                candidates(["合成甲一", "合成甲二", "合成甲三"]),
                EVALUATED_AT_MS,
            )
            .with_user_terms(vec![term("case-a", "合成甲二", 2.0)]),
        },
        SyntheticCase {
            expected_text: "合成乙三",
            request: RankRequest::new(
                "case-b",
                candidates(["合成乙一", "合成乙二", "合成乙三"]),
                EVALUATED_AT_MS,
            )
            .with_ranker_weights(vec![weight("case-b", "合成乙三", 12, 0.0)]),
        },
        SyntheticCase {
            expected_text: "合成丙二",
            request: RankRequest::new(
                "case-c",
                candidates(["合成丙一", "合成丙二", "合成丙三"]),
                EVALUATED_AT_MS,
            )
            .with_ranker_weights(vec![weight("case-c", "合成丙一", 0, 3.0)]),
        },
        SyntheticCase {
            expected_text: "合成丁一",
            request: RankRequest::new(
                "case-d",
                candidates(["合成丁一", "合成丁二", "合成丁三"]),
                EVALUATED_AT_MS,
            )
            .with_user_terms(vec![term("case-d", "合成丁二", 4.0)])
            .with_ranker_weights(vec![weight("case-d", "合成丁二", 30, 0.0)])
            .with_deleted_terms(vec![DeletedTermSummary::new(
                "case-d",
                "合成丁二",
                None::<String>,
            )]),
        },
        SyntheticCase {
            expected_text: "合成戊二",
            request: RankRequest::new(
                "case-e",
                candidates(["合成戊一", "合成戊二", "合成戊三"]),
                EVALUATED_AT_MS,
            ),
        },
    ]
}

fn candidates<const N: usize>(texts: [&str; N]) -> Vec<Candidate> {
    texts.into_iter().map(Candidate::new).collect()
}

fn term(input_code: &str, text: &str, weight: f64) -> UserTerm {
    UserTerm {
        id: 1,
        text: text.to_owned(),
        reading: None,
        input_code: input_code.to_owned(),
        source: TermSource::EngineSelection,
        weight,
        status: TermStatus::Active,
        created_at_ms: 1,
        updated_at_ms: EVALUATED_AT_MS,
        last_used_at_ms: Some(EVALUATED_AT_MS),
        restored_at_ms: None,
        import_batch_id: None,
    }
}

fn weight(input_code: &str, text: &str, frequency: i64, negative_score: f64) -> RankerWeight {
    RankerWeight {
        input_code: input_code.to_owned(),
        text: text.to_owned(),
        reading: None,
        frequency,
        last_used_at_ms: Some(EVALUATED_AT_MS),
        negative_score,
        context_kind: "general".to_owned(),
    }
}
