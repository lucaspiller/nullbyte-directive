.PHONY: help fmt fmt-check clippy test coverage fuzz conformance

help:
	@echo "Available targets: fmt fmt-check clippy test coverage fuzz conformance"

fmt:
	cargo fmt --all
	yarn -s fmt

fmt-check:
	cargo fmt --all -- --check
	yarn -s fmt:check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

coverage:
	@if command -v cargo-llvm-cov >/dev/null 2>&1; then \
		cargo llvm-cov --workspace --lcov --output-path target/llvm-cov/lcov.info; \
		echo "Coverage report written to target/llvm-cov/lcov.info"; \
	else \
		echo "cargo-llvm-cov is not installed. Install with: cargo install cargo-llvm-cov"; \
	fi

fuzz:
	@if command -v cargo-fuzz >/dev/null 2>&1; then \
		echo "cargo-fuzz detected. Add targets under crates/emulator-core/fuzz to run campaigns."; \
		cargo fuzz list || true; \
	else \
		echo "cargo-fuzz is not installed. Install with: cargo install cargo-fuzz"; \
	fi

conformance:
	cargo test -p emulator-core conformance -- --nocapture
