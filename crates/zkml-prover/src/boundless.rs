//! The Boundless market as a [`Market`](crate::remote::Market).
//!
//! Configuration comes from the environment only, so the private key never
//! appears on a command line or in shell history:
//!
//! | Variable | Meaning |
//! | -------- | ------- |
//! | `BOUNDLESS_RPC_URL` | RPC of the chain the market settles on (Base, Base Sepolia, Ethereum Sepolia) |
//! | `BOUNDLESS_PRIVATE_KEY` | Requestor key, 0x-prefixed hex; it pays for the proof |
//! | `BOUNDLESS_PROGRAM_URL` | Public URL of the guest ELF provers download |
//! | `BOUNDLESS_MAX_PRICE` | Ceiling for the offer, e.g. `0.0005 ETH` (default) |
//! | `BOUNDLESS_POLL_SECS` | Seconds between fulfillment checks (default 10) |
//!
//! The request asks for a standalone Groth16 proof (`with_groth16_proof`);
//! Boundless's default, a Merkle inclusion proof against an aggregated root,
//! is not something the Soroban contract can verify.

use std::cell::Cell;
use std::str::FromStr;
use std::time::Duration;

use boundless_market::input::GuestEnv;
use boundless_market::request_builder::OfferParams;
use boundless_market::Client;

use crate::prover::ProveError;
use crate::remote::{Fulfilled, Market, RemoteRequest};

/// Where and how to reach the market. `Debug` leaves the key out.
pub struct BoundlessConfig {
    pub rpc_url: url::Url,
    private_key: String,
    pub program_url: url::Url,
    pub max_price: String,
    pub poll_interval: Duration,
}

impl std::fmt::Debug for BoundlessConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundlessConfig")
            .field("rpc_url", &self.rpc_url.as_str())
            .field("private_key", &"<redacted>")
            .field("program_url", &self.program_url.as_str())
            .field("max_price", &self.max_price)
            .field("poll_interval", &self.poll_interval)
            .finish()
    }
}

fn env(name: &str) -> Result<String, ProveError> {
    std::env::var(name).map_err(|_| {
        ProveError::RemoteBackend(format!("{name} is not set; see `zkml_prover::boundless`"))
    })
}

fn url(name: &str) -> Result<url::Url, ProveError> {
    let value = env(name)?;
    url::Url::parse(&value).map_err(|e| ProveError::RemoteBackend(format!("{name}: {e}")))
}

impl BoundlessConfig {
    /// Read the configuration from the environment.
    pub fn from_env() -> Result<Self, ProveError> {
        Ok(Self {
            rpc_url: url("BOUNDLESS_RPC_URL")?,
            private_key: env("BOUNDLESS_PRIVATE_KEY")?,
            program_url: url("BOUNDLESS_PROGRAM_URL")?,
            max_price: std::env::var("BOUNDLESS_MAX_PRICE").unwrap_or_else(|_| "0.0005 ETH".into()),
            poll_interval: Duration::from_secs(
                std::env::var("BOUNDLESS_POLL_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10),
            ),
        })
    }
}

/// Build a market client from the configuration. A macro rather than a
/// function, because the builder's return type names private generic types.
macro_rules! client {
    ($config:expr) => {
        Client::builder()
            .with_rpc_url($config.rpc_url.clone())
            .with_private_key_str(&$config.private_key)
            .map_err(|e| ProveError::RemoteBackend(format!("boundless client: {e}")))?
            .build()
            .await
            .map_err(|e| ProveError::RemoteBackend(format!("boundless client: {e}")))?
    };
}

/// The real market client. Blocking, on a private Tokio runtime.
pub struct BoundlessMarket {
    config: BoundlessConfig,
    runtime: tokio::runtime::Runtime,
    expires_at: Cell<u64>,
}

impl BoundlessMarket {
    pub fn new(config: BoundlessConfig) -> Result<Self, ProveError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| ProveError::RemoteBackend(format!("tokio runtime: {e}")))?;
        Ok(Self {
            config,
            runtime,
            expires_at: Cell::new(0),
        })
    }
}

impl Market for BoundlessMarket {
    fn submit(&self, request: &RemoteRequest<'_>) -> Result<String, ProveError> {
        let failed = |e: String| ProveError::RemoteBackend(format!("boundless submit: {e}"));
        self.runtime.block_on(async {
            let client = client!(self.config);
            // The guest reads model, inputs and opening, in that order, as
            // `build_env` writes them for local proving.
            let env: GuestEnv = GuestEnv::builder()
                .write(request.model)
                .and_then(|b| b.write(&request.inputs.to_vec()))
                .and_then(|b| b.write(request.opening))
                .map_err(|e| failed(format!("guest input: {e}")))?
                .build_env();
            let max_price =
                boundless_market::price_oracle::Amount::from_str(&self.config.max_price)
                    .map_err(|e| failed(format!("BOUNDLESS_MAX_PRICE: {e:?}")))?;
            let offer = OfferParams::builder()
                .max_price(max_price)
                .build()
                .map_err(|e| failed(e.to_string()))?;
            let params = client
                .new_request()
                // A `Url` converts into a `Url` infallibly.
                .with_program_url(self.config.program_url.clone())
                .unwrap_or_else(|never| match never {})
                .with_image_id(risc0_zkvm::sha::Digest::from(request.image_id))
                .with_env(env)
                .with_offer(offer)
                .with_groth16_proof();
            let (id, expires_at) = client
                .submit_onchain(params)
                .await
                .map_err(|e| failed(e.to_string()))?;
            self.expires_at.set(expires_at);
            Ok(format!("{id:#x}"))
        })
    }

    fn wait(&self, request_id: &str, timeout: Duration) -> Result<Fulfilled, ProveError> {
        let id = alloy_primitives_u256(request_id)?;
        self.runtime.block_on(async {
            let client = client!(self.config);
            let waited = tokio::time::timeout(
                timeout,
                client.wait_for_request_fulfillment(
                    id,
                    self.config.poll_interval,
                    self.expires_at.get(),
                ),
            )
            .await
            .map_err(|_| ProveError::RemoteTimeout {
                request_id: request_id.to_string(),
                waited_secs: timeout.as_secs(),
            })?;
            let fulfillment = waited.map_err(|e| ProveError::RemoteUnfulfilled {
                request_id: request_id.to_string(),
                detail: e.to_string(),
            })?;
            let data = fulfillment
                .data()
                .map_err(|e| ProveError::RemoteWrongProofType {
                    request_id: request_id.to_string(),
                    detail: format!("fulfillment data: {e}"),
                })?;
            let journal = data
                .journal()
                .ok_or_else(|| ProveError::RemoteWrongProofType {
                    request_id: request_id.to_string(),
                    detail: "the fulfillment carries no journal".into(),
                })?;
            Ok(Fulfilled {
                seal: fulfillment.seal.to_vec(),
                journal: journal.to_vec(),
            })
        })
    }
}

fn alloy_primitives_u256(
    request_id: &str,
) -> Result<boundless_market::alloy::primitives::U256, ProveError> {
    boundless_market::alloy::primitives::U256::from_str(request_id)
        .map_err(|e| ProveError::RemoteBackend(format!("request id {request_id}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_the_key() {
        let config = BoundlessConfig {
            rpc_url: url::Url::parse("https://sepolia.base.org").unwrap(),
            private_key: "0x1cee2499e12204c2ed600d780a22a67b3c5fff3310d984cca1f24983d565265c"
                .into(),
            program_url: url::Url::parse("https://example.com/guest.elf").unwrap(),
            max_price: "0.0005 ETH".into(),
            poll_interval: Duration::from_secs(10),
        };
        let shown = format!("{config:?}");
        assert!(!shown.contains("1cee2499"), "{shown}");
        assert!(shown.contains("<redacted>"));
    }
}
