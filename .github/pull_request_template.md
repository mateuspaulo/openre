## Description
Describe your changes and the rationale behind them.

## Type of Change
- [ ] Bug fix (non-breaking change fixing an issue)
- [ ] New feature (e.g. new file carver, filesystem parser)
- [ ] Documentation update
- [ ] Security / Privacy hardening

## Safety & Quality Checklist
- [ ] My code adheres to the project's styling (`cargo fmt`)
- [ ] There are no compiler or clippy warnings (`cargo clippy -- -D warnings`)
- [ ] I have added unit or integration tests for new functionality
- [ ] All tests pass locally (`cargo test`)
- [ ] **Privacy Check:** No telemetry, tracking, or network dependencies were added to recovery modules
- [ ] **Safety Check:** No write operations are directed at the source scan device
- [ ] **Hygiene Check:** No personal files, real disk images, or secrets are committed
