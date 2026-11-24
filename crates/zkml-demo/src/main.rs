//! zkml-demo: the KYC demo, end to end, with a metrics report.
//!
//! 1. Load the model (`.onnx` or the JSON format) and run it natively.
//! 2. Prove it with a real Groth16 receipt, or take a pre-built `--bundle`.
//! 3. Deploy the verifier and register the guest, unless `--contract` names one.
//! 4. Submit `verify_receipt`, read `get_result` back, and compare it with the
//!    native prediction.
//! 5. Report seal size, stage timings and the fee, and mark each of the
//!    roadmap's Phase 1 criteria pass or fail.
//!
//! Contract calls go through the `stellar` CLI. Real proving needs the
//! `groth16` feature and x86_64 Linux with Docker; elsewhere pass `--bundle`
//! with a bundle proved on Linux, and `--risc0-params` from `export-vk`.

mod report;
mod stellar;

use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser;
use serde_json::Value;
use zkml_common::bundle::VerificationBundleV2;
use zkml_common::commitment::to_hex;
use zkml_common::inference::try_run_inference_with_decision;
use zkml_common::journal::JournalV1;
use zkml_prover::cli::{load_model, parse_inputs, read_opening};
use zkml_prover::prover::{bundle_v2_from_json, model_commitment};

use report::Report;
use stellar::Stellar;

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Run the zkml-soroban demo end to end and report on it"
)]
struct Args {
    /// Model file: `.onnx`, or the JSON exchange format.
    #[arg(short, long)]
    model: PathBuf,
    /// Comma-separated input features, e.g. "0.6,0.4,0.0".
    #[arg(short, long, allow_hyphen_values = true)]
    input: String,
    /// The model's opening (64 hex characters), as for `zkml-prover commit`.
    #[arg(long)]
    opening: PathBuf,
    /// Submit this pre-built bundle instead of proving in this run.
    #[arg(long)]
    bundle: Option<PathBuf>,
    /// Use this deployed verifier instead of deploying a new one.
    #[arg(long)]
    contract: Option<String>,
    /// RISC Zero parameters from `zkml-prover export-vk --format json`, needed
    /// to register the guest on a new contract when this build cannot export
    /// them itself (without the `zkvm` feature).
    #[arg(long)]
    risc0_params: Option<PathBuf>,
    /// The contract WASM to deploy.
    #[arg(
        long,
        default_value = "target/wasm32v1-none/contract/zkml_verifier.wasm"
    )]
    wasm: String,
    /// `stellar` network name.
    #[arg(long, default_value = "testnet")]
    network: String,
    /// `stellar` identity that deploys, administers and submits.
    #[arg(long, default_value = "zkml-demo")]
    source: String,
    /// RPC endpoint for reading the fee; defaults from the network.
    #[arg(long)]
    rpc_url: Option<String>,
    /// Also write the report as JSON here.
    #[arg(long)]
    report: Option<PathBuf>,
}

fn main() {
    if let Err(e) = run(Args::parse()) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    let version = Stellar::check_installed()?;
    eprintln!("using {version}");
    let chain = Stellar::new(&args.network, &args.source, args.rpc_url.as_deref());
    let mut report = Report {
        network: args.network.clone(),
        ..Default::default()
    };

    // 1. Native inference: the value the chain has to agree with.
    let model = load_model(&args.model).map_err(|e| e.to_string())?;
    let inputs = parse_inputs(&args.input).map_err(|e| e.to_string())?;
    let opening = read_opening(&args.opening).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let (output, class_label) =
        try_run_inference_with_decision(&model, &inputs).map_err(|e| e.to_string())?;
    report.stage("native", started.elapsed());
    report.native_output_raw = output.value;
    report.native_class_label = class_label;
    let model_hash = to_hex(&model_commitment(&model, &opening));
    eprintln!(
        "native output {} (raw {}), class {class_label}",
        output.dequantize(),
        output.value
    );

    // 2. The bundle: proved here, or read from --bundle.
    let bundle = match &args.bundle {
        Some(path) => read_bundle(path)?,
        None => {
            let started = Instant::now();
            let bundle = prove(&model, &inputs, &opening)?;
            report.stage("prove", started.elapsed());
            report.proved_here = true;
            bundle
        }
    };
    check_bundle(&bundle, &model_hash, output.value, class_label)?;
    report.image_id = hex(&bundle.image_id);
    report.seal_bytes = bundle.seal.len();
    report.journal_bytes = bundle.journal.len();

    // 3. The contract.
    report.contract_id = match &args.contract {
        Some(id) => id.clone(),
        None => {
            let started = Instant::now();
            let id = deploy(&chain, &args, &model_hash, &bundle)?;
            report.stage("deploy", started.elapsed());
            id
        }
    };

    // 4. Submit and read back.
    let started = Instant::now();
    let submitted = chain.invoke(
        &report.contract_id,
        "verify_receipt",
        &[
            "--seal",
            &hex(&bundle.seal),
            "--journal",
            &hex(&bundle.journal),
        ],
        true,
    )?;
    report.stage("submit", started.elapsed());
    report.tx_hash = submitted.tx_hash.clone();
    if let Some(hash) = &submitted.tx_hash {
        match chain.fee_charged(hash) {
            Ok(fee) => report.fee_stroops = Some(fee),
            Err(e) => eprintln!("warning: could not read the fee: {e}"),
        }
    }
    let record = chain.invoke(&report.contract_id, "get_result", &[], false)?;
    let (chain_output, chain_label) = parse_record(&record.stdout)?;
    report.chain_output_raw = Some(chain_output);
    report.chain_class_label = Some(chain_label);

    // 5. Report.
    report.evaluate();
    report.print();
    if let Some(path) = &args.report {
        let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_bundle(path: &Path) -> Result<VerificationBundleV2, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    bundle_v2_from_json(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Refuse a bundle the contract would refuse, or one that disagrees with the
/// native run, before spending a transaction on it.
fn check_bundle(
    bundle: &VerificationBundleV2,
    model_hash: &str,
    output: i64,
    class_label: i64,
) -> Result<(), String> {
    let raw: [u8; 96] = bundle
        .journal
        .as_slice()
        .try_into()
        .map_err(|_| format!("the journal is {} bytes, not 96", bundle.journal.len()))?;
    let journal =
        JournalV1::decode(&raw).map_err(|e| format!("the journal does not decode: {e:?}"))?;
    if hex(&journal.model_hash) != model_hash {
        return Err(format!(
            "the bundle is for model commitment {}, not this model under this opening ({model_hash})",
            hex(&journal.model_hash)
        ));
    }
    if journal.output != output || journal.class_label != class_label {
        return Err(format!(
            "the bundle proves output {} class {}, but the model gives {output} class {class_label} on this input",
            journal.output, journal.class_label
        ));
    }
    Ok(())
}

#[cfg(feature = "groth16")]
fn prove(
    model: &zkml_common::models::Model,
    inputs: &[zkml_common::fixed_point::FixedPoint],
    opening: &zkml_common::commitment::Opening,
) -> Result<VerificationBundleV2, String> {
    use zkml_prover::prover::{bundle_from_output, prove_groth16, ProverBackend};
    let out =
        prove_groth16(model, inputs, opening, &ProverBackend::Local).map_err(|e| e.to_string())?;
    bundle_from_output(&out).map_err(|e| e.to_string())
}

#[cfg(not(feature = "groth16"))]
fn prove(
    _: &zkml_common::models::Model,
    _: &[zkml_common::fixed_point::FixedPoint],
    _: &zkml_common::commitment::Opening,
) -> Result<VerificationBundleV2, String> {
    Err(
        "this build cannot prove: pass --bundle, or build with --features groth16 \
         (x86_64 Linux with Docker)"
            .into(),
    )
}

/// The RISC Zero parameters to register: from --risc0-params, or exported by
/// this build. The image id is always the bundle's, so a bundle proved on
/// another machine registers the guest that actually proved it.
fn risc0_params(args: &Args) -> Result<Value, String> {
    if let Some(path) = &args.risc0_params {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        return serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()));
    }
    export_params()
}

#[cfg(feature = "zkvm")]
fn export_params() -> Result<Value, String> {
    let mut out = Vec::new();
    zkml_prover::cli::cmd_export_vk("json", None, &mut out).map_err(|e| e.to_string())?;
    serde_json::from_slice(&out).map_err(|e| e.to_string())
}

#[cfg(not(feature = "zkvm"))]
fn export_params() -> Result<Value, String> {
    Err(
        "deploying needs the RISC Zero parameters: pass --risc0-params from \
         `zkml-prover export-vk --format json`, or build with --features zkvm"
            .into(),
    )
}

fn deploy(
    chain: &Stellar,
    args: &Args,
    model_hash: &str,
    bundle: &VerificationBundleV2,
) -> Result<String, String> {
    let params = risc0_params(args)?;
    let admin = chain.address()?;
    let id = chain.deploy(&args.wasm, &admin, model_hash)?;
    eprintln!("deployed {id}");
    let field = |key: &str| {
        params[key]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("the RISC Zero parameters have no {key}"))
    };
    let config = serde_json::json!({
        "image_id": hex(&bundle.image_id),
        "control_root": field("control_root")?,
        "bn254_control_id": field("bn254_control_id")?,
        "selector": field("selector")?,
    });
    chain.invoke(
        &id,
        "set_risc0_config",
        &["--config", &config.to_string()],
        true,
    )?;
    chain.invoke(
        &id,
        "set_risc0_vk",
        &["--vk", &params["vk"].to_string()],
        true,
    )?;
    Ok(id)
}

/// `get_result` prints the record as JSON: `output` as the hex of 8
/// little-endian bytes, `class_label` as a number (or a string, depending on
/// the CLI version).
fn parse_record(stdout: &str) -> Result<(i64, i64), String> {
    let record: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("get_result did not print a record ({e}): {stdout}"))?;
    let output_hex = record["output"]
        .as_str()
        .ok_or("the record has no output")?;
    let bytes: Vec<u8> = (0..output_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&output_hex[i..i + 2], 16))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("output is not hex: {e}"))?;
    let output = i64::from_le_bytes(
        bytes
            .as_slice()
            .try_into()
            .map_err(|_| "output is not 8 bytes")?,
    );
    let label = match &record["class_label"] {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
    .ok_or("the record has no class_label")?;
    Ok((output, label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_parses_with_either_label_encoding() {
        let as_number =
            r#"{"class_label":1,"model_hash":"ab","output":"0000010000000000","verified_at":7}"#;
        assert_eq!(parse_record(as_number).unwrap(), (65536, 1));
        let as_string =
            r#"{"class_label":"2","model_hash":"ab","output":"0000020000000000","verified_at":7}"#;
        assert_eq!(parse_record(as_string).unwrap(), (131072, 2));
        assert!(parse_record("null").is_err());
    }
}
