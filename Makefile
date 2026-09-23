.DEFAULT_GOAL := help

## help: List available commands
help:
	@grep -E '^## ' $(MAKEFILE_LIST) | sed 's/^## //'

## unit-tests: Run unit tests for (macro, carburetor) x (backend, client)
unit-tests:
	CARGO_TARGET_DIR=target/backend cargo test -p carburetor-macro
	CARGO_TARGET_DIR=target/backend cargo test -p carburetor --features diesel/postgres
	CARGO_TARGET_DIR=target/client CARBURETOR_TARGET=client cargo test -p carburetor-macro
	CARGO_TARGET_DIR=target/client CARBURETOR_TARGET=client cargo test -p carburetor --features diesel/sqlite --features migration

## fmt: Check formatting
fmt:
	cargo fmt --all --check

## clippy-backend: Lint the backend variant (macro + carburetor + example)
clippy-backend:
	CARGO_TARGET_DIR=target/backend cargo clippy --no-deps -p carburetor-macro --all-targets -- -D warnings
	CARGO_TARGET_DIR=target/backend cargo clippy --no-deps -p carburetor --features diesel/postgres --all-targets -- -D warnings
	CARGO_TARGET_DIR=target/backend cargo clippy --no-deps -p carburetor-example --features backend --example simple-backend -- -D warnings

## clippy-client: Lint the client variant (macro + carburetor + example)
clippy-client:
	CARGO_TARGET_DIR=target/client CARBURETOR_TARGET=client cargo clippy --no-deps -p carburetor-macro --all-targets -- -D warnings
	CARGO_TARGET_DIR=target/client CARBURETOR_TARGET=client cargo clippy --no-deps -p carburetor --features diesel/sqlite --features migration --all-targets -- -D warnings
	CARGO_TARGET_DIR=target/client CARBURETOR_TARGET=client cargo clippy --no-deps -p carburetor-example --features client --example simple-client -- -D warnings

## e2e: Build the test backend, then run the e2e test suite
e2e:
	CARGO_TARGET_DIR=target/backend cargo build -p sample-test-backend
	CARGO_TARGET_DIR=target/client CARBURETOR_TARGET=client cargo test -p e2e-test

## all: Run all checks, then print a summary of failures
all:
	@tmp=$$(mktemp -d); \
	checks='fmt clippy-backend unit-tests clippy-client e2e'; \
	run_check() { \
	  label=$$1; shift; \
	  ( "$$@" >"$$tmp/$$label.log" 2>&1; echo $$? >"$$tmp/$$label.code" ) & \
	}; \
	print_results() { \
	  remaining="$$checks"; \
	  while [ -n "$$remaining" ]; do \
	    next=''; \
	    for label in $$remaining; do \
	      if [ -f "$$tmp/$$label.code" ]; then \
	        if [ "$$(cat "$$tmp/$$label.code")" -eq 0 ]; then \
	          printf 'PASS: %s\n' "$$label"; \
	        else \
	          printf 'FAIL: %s (see %s/%s.log)\n' "$$label" "$$tmp" "$$label"; \
	          echo "$$label" >>"$$tmp/failures"; \
	        fi; \
	      else \
	        next="$$next $$label"; \
	      fi; \
	    done; \
	    remaining=$$next; \
	    [ -n "$$remaining" ] && sleep 0.2; \
	  done; \
	}; \
	print_results & \
	run_check 'fmt' $(MAKE) fmt; \
	run_check 'clippy-backend' $(MAKE) clippy-backend; \
	run_check 'clippy-client' $(MAKE) clippy-client; \
	run_check 'unit-tests' $(MAKE) unit-tests; \
	run_check 'e2e' $(MAKE) e2e; \
	wait; \
	if [ -f "$$tmp/failures" ]; then \
	  printf '\nFailed checks:%s\n' "$$(cat "$$tmp/failures" | tr '\n' ' ')"; \
	  exit 1; \
	fi; \
	rm -rf "$$tmp"; \
	printf '\nAll checks passed\n'
