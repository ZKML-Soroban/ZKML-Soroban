//! Remote proving against a mock market: the request lifecycle and every check
//! on what a market returns, with no network and no funds.
//!
//! The fulfilled case replays the golden receipt in
//! `crates/zkml-verifier/testdata/` (`credit_lr.json` on `0.5,0.2,0.9,0.1`
//! under the public test opening), so the bundle `prove_remote` assembles is
//! checked by the real Groth16 verifier.

#![cfg(feature = "zkvm")]

use std::cell::RefCell;
use std::path::Path;
use std::time::Duration;

use zkml_prover::cli::{load_model, parse_inputs};
use zkml_prover::prover::ProveError;
use zkml_prover::remote::{prove_remote, Fulfilled, Market, RemoteRequest};

const OPENING: [u8; 32] = [0x2a; 32];

fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

/// The golden receipt: image id, seal, journal.
fn golden() -> ([u8; 32], Vec<u8>, Vec<u8>) {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../zkml-verifier/testdata/groth16_bundle.json");
    let bundle: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let field = |k: &str| unhex(bundle[k].as_str().unwrap());
    (
        field("image_id").try_into().unwrap(),
        field("seal"),
        field("journal"),
    )
}

/// How the mock answers `wait` for a request id.
type Reply = Box<dyn Fn(&str) -> Result<Fulfilled, ProveError>>;

/// A market that records what it was asked and answers with `reply`.
struct MockMarket {
    reply: Reply,
    submitted: RefCell<Vec<([u8; 32], u64)>>,
}

impl MockMarket {
    fn new(reply: impl Fn(&str) -> Result<Fulfilled, ProveError> + 'static) -> Self {
        Self {
            reply: Box::new(reply),
            submitted: RefCell::new(Vec::new()),
        }
    }
}

impl Market for MockMarket {
    fn submit(&self, request: &RemoteRequest<'_>) -> Result<String, ProveError> {
        self.submitted
            .borrow_mut()
            .push((request.image_id, request.cycles));
        Ok("0x2a".into())
    }
    fn wait(&self, request_id: &str, _timeout: Duration) -> Result<Fulfilled, ProveError> {
        (self.reply)(request_id)
    }
}

fn run(market: &MockMarket) -> Result<zkml_common::bundle::VerificationBundleV2, ProveError> {
    let model = load_model(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/models/credit_lr.json"),
    )
    .unwrap();
    let inputs = parse_inputs("0.5,0.2,0.9,0.1").unwrap();
    let (image_id, _, _) = golden();
    prove_remote(
        &model,
        &inputs,
        &OPENING,
        image_id,
        market,
        Duration::from_secs(1),
    )
}

#[test]
fn a_fulfilled_request_becomes_a_verified_bundle() {
    let (image_id, seal, journal) = golden();
    let (s, j) = (seal.clone(), journal.clone());
    let market = MockMarket::new(move |_| {
        Ok(Fulfilled {
            seal: s.clone(),
            journal: j.clone(),
        })
    });
    let bundle = run(&market).expect("the golden receipt passes every check");
    assert_eq!(bundle.seal, seal);
    assert_eq!(bundle.journal, journal);
    assert_eq!(bundle.image_id, image_id);
    let submitted = market.submitted.borrow();
    assert_eq!(submitted.len(), 1);
    assert_eq!(submitted[0].0, image_id, "the request names the guest");
    assert!(
        submitted[0].1 > 0,
        "the request carries the local cycle count"
    );
}

#[test]
fn an_unfulfilled_request_says_which_one() {
    let market = MockMarket::new(|id| {
        Err(ProveError::RemoteUnfulfilled {
            request_id: id.into(),
            detail: "expired".into(),
        })
    });
    match run(&market) {
        Err(ProveError::RemoteUnfulfilled { request_id, .. }) => assert_eq!(request_id, "0x2a"),
        other => panic!("expected RemoteUnfulfilled, got {:?}", other.err()),
    }
}

#[test]
fn a_timeout_says_which_request_to_look_up() {
    let market = MockMarket::new(|id| {
        Err(ProveError::RemoteTimeout {
            request_id: id.into(),
            waited_secs: 1,
        })
    });
    match run(&market) {
        Err(ProveError::RemoteTimeout { request_id, .. }) => assert_eq!(request_id, "0x2a"),
        other => panic!("expected RemoteTimeout, got {:?}", other.err()),
    }
}

#[test]
fn a_seal_of_the_wrong_length_is_refused() {
    let (_, seal, journal) = golden();
    let market = MockMarket::new(move |_| {
        Ok(Fulfilled {
            seal: seal[..200].to_vec(),
            journal: journal.clone(),
        })
    });
    assert!(matches!(
        run(&market),
        Err(ProveError::RemoteWrongProofType { .. })
    ));
}

#[test]
fn a_seal_from_another_risc0_version_is_refused() {
    let (_, mut seal, journal) = golden();
    seal[0] ^= 0xff; // the selector
    let market = MockMarket::new(move |_| {
        Ok(Fulfilled {
            seal: seal.clone(),
            journal: journal.clone(),
        })
    });
    assert!(matches!(
        run(&market),
        Err(ProveError::RemoteWrongProofType { .. })
    ));
}

#[test]
fn a_journal_that_disagrees_with_native_inference_is_refused() {
    let (_, seal, mut journal) = golden();
    journal[72] ^= 0x01; // a byte of the output
    let market = MockMarket::new(move |_| {
        Ok(Fulfilled {
            seal: seal.clone(),
            journal: journal.clone(),
        })
    });
    assert!(matches!(run(&market), Err(ProveError::JournalMismatch(_))));
}
