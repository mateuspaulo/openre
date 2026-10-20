# Contributing to OpenRe

Thank you for your interest in contributing to **OpenRe**! OpenRe is committed to building a transparent, privacy-first, professional open-source file recovery ecosystem.

---

## 1. Code of Conduct

All contributors and participants must adhere to our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## 2. Privacy & Safety Commitments

Contributors must uphold OpenRe's core architectural tenets:
1. **Zero Telemetry:** Never introduce networking, remote logging, or tracking dependencies into recovery modules.
2. **Read-Only Source Operations:** Never add write operations directed at the source drive being scanned.
3. **Synthetic Test Data Only:** Never commit real personal photos, sensitive files, or real disk dumps to the repository. All test fixtures must be synthetically generated.
4. **Memory Safety:** Prefer safe Rust. Any `unsafe` blocks required for low-level OS device IOCTLs must include detailed safety documentation and bounds checks.

---

## 3. Development Workflow

1. Fork the repository and clone your fork.
2. Create a feature branch: `git checkout -b feat/my-improvement`.
3. Verify your changes pass all linters and tests:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   ```
4. Commit your changes using clear, descriptive English commit messages.
5. Push your branch and open a Pull Request.
