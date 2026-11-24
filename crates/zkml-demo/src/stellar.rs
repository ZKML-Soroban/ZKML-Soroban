//! The contract calls, through the `stellar` CLI.
//!
//! The CLI already builds, simulates, signs and submits Soroban transactions,
//! and it is what the deployment guide uses, so the runner drives it instead
//! of re-implementing transaction assembly. Every command is printed before it
//! runs, as `examples/kyc-demo/deploy.sh` does.

use std::process::Command;

use serde_json::Value;

/// Where and as whom to call the contract.
pub struct Stellar {
    pub network: String,
    pub source: String,
    pub rpc_url: String,
}

/// What a submitted call returned.
pub struct Submitted {
    pub stdout: String,
    pub tx_hash: Option<String>,
}

impl Stellar {
    pub fn new(network: &str, source: &str, rpc_url: Option<&str>) -> Self {
        let rpc_url = rpc_url.map(str::to_string).unwrap_or_else(|| {
            match network {
                "testnet" => "https://soroban-testnet.stellar.org",
                "mainnet" => "https://mainnet.sorobanrpc.com",
                _ => "http://localhost:8000/rpc",
            }
            .to_string()
        });
        Self {
            network: network.to_string(),
            source: source.to_string(),
            rpc_url,
        }
    }

    /// Fail early, with the install link, when the CLI is missing.
    pub fn check_installed() -> Result<String, String> {
        let out = Command::new("stellar")
            .arg("--version")
            .output()
            .map_err(|_| {
                "the stellar CLI is not on PATH: https://developers.stellar.org/docs/tools/cli"
                    .to_string()
            })?;
        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .to_string())
    }

    pub fn address(&self) -> Result<String, String> {
        run(&["keys", "address", &self.source]).map(|s| s.stdout.trim().to_string())
    }

    /// Deploy the contract with its constructor and return the contract id.
    pub fn deploy(&self, wasm: &str, admin: &str, model_hash: &str) -> Result<String, String> {
        let out = run(&[
            "contract",
            "deploy",
            "--wasm",
            wasm,
            "--source-account",
            &self.source,
            "--network",
            &self.network,
            "--",
            "--admin",
            admin,
            "--model_hash",
            model_hash,
        ])?;
        out.stdout
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with('C') && l.len() == 56)
            .map(str::to_string)
            .ok_or_else(|| format!("no contract id in the deploy output: {}", out.stdout))
    }

    /// Call `function` with `args` (already `--name value` pairs). `send`
    /// forces a transaction even when the simulation looks read-only.
    pub fn invoke(
        &self,
        id: &str,
        function: &str,
        args: &[&str],
        send: bool,
    ) -> Result<Submitted, String> {
        let mut argv = vec![
            "contract",
            "invoke",
            "--id",
            id,
            "--source-account",
            &self.source,
            "--network",
            &self.network,
        ];
        if send {
            argv.push("--send=yes");
        }
        argv.push("--");
        argv.push(function);
        argv.extend_from_slice(args);
        run(&argv)
    }

    /// The fee charged for a transaction, in stroops, from `getTransaction`.
    /// `resultXdr` is a `TransactionResult`, which starts with `feeCharged` as
    /// a big-endian `int64`, so no XDR library is needed for it.
    pub fn fee_charged(&self, tx_hash: &str) -> Result<i64, String> {
        use base64::Engine as _;
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "getTransaction",
            "params": { "hash": tx_hash },
        });
        let response: Value = reqwest::blocking::Client::new()
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .and_then(|r| r.json())
            .map_err(|e| format!("getTransaction on {}: {e}", self.rpc_url))?;
        let xdr = response["result"]["resultXdr"]
            .as_str()
            .ok_or_else(|| format!("getTransaction returned no resultXdr: {response}"))?;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(xdr)
            .map_err(|e| format!("resultXdr is not base64: {e}"))?;
        let fee: [u8; 8] = raw
            .get(..8)
            .and_then(|b| b.try_into().ok())
            .ok_or("resultXdr is shorter than 8 bytes")?;
        Ok(i64::from_be_bytes(fee))
    }
}

/// Run `stellar` with `args`, echoing the command; stderr is where the CLI
/// reports the transaction hash ("Signing transaction: <hash>").
fn run(args: &[&str]) -> Result<Submitted, String> {
    let shown: Vec<String> = args
        .iter()
        .map(|a| {
            if a.len() > 80 {
                format!("{}...", &a[..77])
            } else {
                a.to_string()
            }
        })
        .collect();
    eprintln!("$ stellar {}", shown.join(" "));
    let out = Command::new("stellar")
        .args(args)
        .output()
        .map_err(|e| format!("running stellar: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        return Err(format!(
            "stellar {} failed:\n{stderr}",
            args.first().unwrap_or(&"")
        ));
    }
    Ok(Submitted {
        stdout,
        tx_hash: tx_hash_from(&stderr),
    })
}

/// The 64-hex transaction hash in the CLI's "Signing transaction: <hash>" line.
pub fn tx_hash_from(stderr: &str) -> Option<String> {
    stderr.lines().find_map(|line| {
        let rest = line.split("Signing transaction:").nth(1)?;
        let hash = rest.trim();
        (hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hash_is_read_from_the_signing_line() {
        let stderr = "ℹ️  Signing transaction: 597c62df5dce10567e9c0b0f30c8b22d2e0c918005dacc5b1e9ff34e396733b9\n🔗 https://stellar.expert/...";
        assert_eq!(
            tx_hash_from(stderr).as_deref(),
            Some("597c62df5dce10567e9c0b0f30c8b22d2e0c918005dacc5b1e9ff34e396733b9")
        );
        assert_eq!(tx_hash_from("Simulation identified as read-only."), None);
    }

    #[test]
    fn default_rpc_urls_follow_the_network() {
        assert!(Stellar::new("testnet", "me", None)
            .rpc_url
            .contains("testnet"));
        assert!(Stellar::new("local", "me", None)
            .rpc_url
            .contains("localhost"));
        assert_eq!(
            Stellar::new("testnet", "me", Some("http://x")).rpc_url,
            "http://x"
        );
    }
}
