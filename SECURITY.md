# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in zkml-soroban, please report it
responsibly. **Do not open a public GitHub issue.**

### Contact

Send a detailed report to the project maintainers via a private channel.
Include the following information:

- Description of the vulnerability and its potential impact.
- Steps to reproduce the issue.
- Any proof-of-concept code, if available.
- Your suggested fix, if any.

### Response Timeline

- **Acknowledgment**: Within 48 hours of receiving your report.
- **Assessment**: Within 7 days, we will confirm the vulnerability and
  its severity.
- **Fix and disclosure**: We aim to release a fix within 30 days of
  confirmation. A coordinated disclosure will follow once the fix is
  deployed.

## Scope

The following components are in scope for security reports:

- **On-chain verifier contract** (`zkml-verifier`): Proof verification
  logic, storage access control, and contract initialization.
- **Proof generation pipeline** (`zkml-prover`): Correctness of proof
  construction, serialization, and public input encoding.
- **Cryptographic operations**: Fixed-point arithmetic overflow, hash
  commitment integrity, and BN254 curve point handling.

## Out of Scope

- Vulnerabilities in upstream dependencies (Soroban SDK, RISC Zero)
  should be reported to their respective maintainers.
- Issues related to model accuracy or ML performance are not security
  vulnerabilities.

## Acknowledgments

We appreciate the security research community and will credit reporters
(with their consent) in release notes.
