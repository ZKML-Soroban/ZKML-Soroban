//! The run's measurements and the roadmap's Phase 1 success criteria.

use serde::Serialize;
use std::time::Duration;

/// One timed stage of the run.
#[derive(Debug, Serialize)]
pub struct Stage {
    pub name: &'static str,
    pub millis: u128,
}

/// Pass, fail, or not measured in this run (with the reason).
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail,
    NotMeasured(String),
}

#[derive(Debug, Serialize)]
pub struct Criterion {
    pub name: &'static str,
    pub target: &'static str,
    pub measured: String,
    pub outcome: Outcome,
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub network: String,
    pub contract_id: String,
    pub image_id: String,
    pub stages: Vec<Stage>,
    pub seal_bytes: usize,
    pub journal_bytes: usize,
    pub tx_hash: Option<String>,
    pub fee_stroops: Option<i64>,
    pub native_output_raw: i64,
    pub native_class_label: i64,
    pub chain_output_raw: Option<i64>,
    pub chain_class_label: Option<i64>,
    pub proved_here: bool,
    pub criteria: Vec<Criterion>,
}

impl Report {
    pub fn stage(&mut self, name: &'static str, took: Duration) {
        self.stages.push(Stage {
            name,
            millis: took.as_millis(),
        });
    }

    fn millis_of(&self, names: &[&str]) -> u128 {
        self.stages
            .iter()
            .filter(|s| names.contains(&s.name))
            .map(|s| s.millis)
            .sum()
    }

    /// The roadmap's criteria: proof under 500 bytes, end to end under 60 s,
    /// and the result verified on chain and equal to the native prediction.
    pub fn evaluate(&mut self) {
        let payload = self.seal_bytes + self.journal_bytes;
        let mut criteria = vec![Criterion {
            name: "proof size",
            target: "under 500 bytes",
            measured: format!(
                "{payload} bytes ({} seal + {} journal)",
                self.seal_bytes, self.journal_bytes
            ),
            outcome: if payload < 500 {
                Outcome::Pass
            } else {
                Outcome::Fail
            },
        }];

        let latency = self.millis_of(&["prove", "submit"]);
        criteria.push(Criterion {
            name: "end-to-end latency",
            target: "under 60 s (prove + submit + verify)",
            measured: format!("{:.1} s", latency as f64 / 1000.0),
            outcome: if !self.proved_here {
                Outcome::NotMeasured(
                    "the proof came from --bundle, so proving time is not in this run".into(),
                )
            } else if latency < 60_000 {
                Outcome::Pass
            } else {
                Outcome::Fail
            },
        });

        let agrees = self.chain_class_label == Some(self.native_class_label)
            && self.chain_output_raw == Some(self.native_output_raw);
        criteria.push(Criterion {
            name: "verified on chain",
            target: "get_result equals the native prediction",
            measured: match (self.chain_class_label, self.chain_output_raw) {
                (Some(label), Some(raw)) => format!("class_label {label}, output {raw}"),
                _ => "no record read back".into(),
            },
            outcome: if agrees { Outcome::Pass } else { Outcome::Fail },
        });
        self.criteria = criteria;
    }

    pub fn print(&self) {
        println!("\n=== zkml-demo report ({})", self.network);
        println!("contract   {}", self.contract_id);
        println!("image id   {}", self.image_id);
        if let Some(hash) = &self.tx_hash {
            println!("tx         {hash}");
        }
        if let Some(fee) = self.fee_stroops {
            println!("fee        {fee} stroops ({:.7} XLM)", fee as f64 / 1e7);
        }
        println!(
            "native     output {} class {}",
            self.native_output_raw, self.native_class_label
        );
        for stage in &self.stages {
            println!(
                "stage      {:<10} {:>10.1} s",
                stage.name,
                stage.millis as f64 / 1000.0
            );
        }
        println!();
        for c in &self.criteria {
            let mark = match &c.outcome {
                Outcome::Pass => "PASS".to_string(),
                Outcome::Fail => "FAIL".to_string(),
                Outcome::NotMeasured(why) => format!("NOT MEASURED ({why})"),
            };
            println!("{:<20} {:<40} {:<40} {mark}", c.name, c.target, c.measured);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(proved_here: bool, prove_ms: u64, label: Option<i64>) -> Report {
        let mut r = Report {
            seal_bytes: 260,
            journal_bytes: 96,
            native_output_raw: 65536,
            native_class_label: 1,
            chain_output_raw: label.map(|_| 65536),
            chain_class_label: label,
            proved_here,
            ..Default::default()
        };
        if proved_here {
            r.stage("prove", Duration::from_millis(prove_ms));
        }
        r.stage("submit", Duration::from_millis(5_000));
        r.evaluate();
        r
    }

    #[test]
    fn a_fast_run_that_agrees_passes_everything() {
        let r = report(true, 20_000, Some(1));
        assert!(
            r.criteria.iter().all(|c| c.outcome == Outcome::Pass),
            "{:?}",
            r.criteria
        );
    }

    #[test]
    fn local_proving_misses_the_latency_target() {
        let r = report(true, 1_986_000, Some(1));
        assert_eq!(r.criteria[1].outcome, Outcome::Fail);
    }

    #[test]
    fn a_prebuilt_bundle_does_not_measure_latency() {
        let r = report(false, 0, Some(1));
        assert!(matches!(r.criteria[1].outcome, Outcome::NotMeasured(_)));
    }

    #[test]
    fn a_different_label_on_chain_fails() {
        let r = report(true, 1_000, Some(2));
        assert_eq!(r.criteria[2].outcome, Outcome::Fail);
        let r = report(true, 1_000, None);
        assert_eq!(r.criteria[2].outcome, Outcome::Fail);
    }
}
