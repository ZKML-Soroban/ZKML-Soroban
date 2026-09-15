---
title: "Contributing"
description: "How to set up the project, follow the coding standards, and preview these docs."
icon: "code-pull-request"
---

The canonical guide is
[CONTRIBUTING.md](https://github.com/ZKML-Soroban/ZKML-Soroban/blob/main/CONTRIBUTING.md)
in the repository root. This page summarizes it.

## Setup

```bash
git clone https://github.com/ZKML-Soroban/ZKML-Soroban.git
cd ZKML-Soroban
cargo build --workspace
cargo test --workspace
```

`rust-toolchain.toml` pins the stable channel, `rustfmt`, `clippy`, and the
`wasm32v1-none` target. The RISC Zero toolchain (`3.0.6`) is needed only for
the `zkvm` feature.

## Before opening a pull request

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo build -p zkml-verifier --target wasm32v1-none --profile contract
```

- Use [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat(prover): ...`, `fix(verifier): ...`, `docs: ...`).
- Add tests for any change to inference, quantization, commitments, or the
  contract. See [Testing](/guides/testing).
- Changes to consensus-critical code need maintainer review and updated
  snapshots. See [Security notes](/security/security-notes#consensus-critical-code).
- Update the docs when public behavior changes.

## Working on the docs

The docs are a [Mintlify](https://mintlify.com) site in `docs/`:

- `docs/docs.json` defines navigation, theme, and links.
- Every page needs `title` and `description` frontmatter.
- Internal links are root-relative without extensions, for example
  `/concepts/architecture`.
- Keep pages as `.md` unless they need Mintlify components (`.mdx`).

Preview locally (requires Node.js):

```bash
cd docs
npx mint dev
```

Check links before pushing:

```bash
cd docs
npx mint broken-links
```

CI runs the same link check.

## Security issues

Report vulnerabilities privately as described in
[SECURITY.md](https://github.com/ZKML-Soroban/ZKML-Soroban/blob/main/SECURITY.md).
