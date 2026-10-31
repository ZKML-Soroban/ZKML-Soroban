//! Builds the zkVM guest and embeds its ELF and image id.
//!
//! By default the guest is compiled with the host's own RISC Zero toolchain.
//! That is fast, but the binary, and so the image id, depends on the machine:
//! the same commit builds a different id elsewhere, and a contract only accepts
//! receipts from the id its admin registered.
//!
//! With `ZKML_GUEST_DOCKER=1` the guest is built inside RISC Zero's pinned
//! builder image instead, from the guest's own `Cargo.lock`, which gives the
//! same image id on every machine. CI and releases build that way, and it is
//! the build whose image id gets registered on chain. See CONTRIBUTING.md,
//! "Changing the zkVM guest".

use std::collections::HashMap;
use std::path::PathBuf;

use risc0_build::{DockerOptionsBuilder, GuestOptionsBuilder};

const GUEST_PACKAGE: &str = "zkml-guest";

/// RISC Zero builder image for the Docker build.
///
/// `risc0-build` 3.0.6 defaults to `r0.1.88.0`, whose Rust 1.88 is older than
/// the workspace MSRV (1.91, which `zkml-common` declares) and than what some
/// guest dependencies need, so that build fails. `r0.1.91.1` ships Rust 1.91.
/// Pinning the tag is part of what makes the image id reproducible: a different
/// toolchain produces a different binary. `RISC0_DOCKER_CONTAINER_TAG`
/// overrides it.
const BUILDER_TAG: &str = "r0.1.91.1";

fn main() {
    println!("cargo:rerun-if-env-changed=ZKML_GUEST_DOCKER");
    let docker = std::env::var("ZKML_GUEST_DOCKER").is_ok_and(|value| value == "1");

    if docker {
        // The Docker build context has to contain every path dependency of the
        // guest, and the guest depends on crates/zkml-common, so it is the
        // workspace root rather than this crate.
        let root =
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo")).join("..");
        let docker_options = DockerOptionsBuilder::default()
            .root_dir(root)
            .docker_container_tag(BUILDER_TAG)
            .build()
            .expect("valid Docker options");
        let guest_options = GuestOptionsBuilder::default()
            .use_docker(docker_options)
            .build()
            .expect("valid guest options");
        risc0_build::embed_methods_with_options(HashMap::from([(GUEST_PACKAGE, guest_options)]));
    } else {
        risc0_build::embed_methods();
    }

    println!(
        "cargo:rustc-env=ZKML_GUEST_BUILD={}",
        if docker { "docker" } else { "local" }
    );
}
