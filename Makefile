.PHONY: help demo demo-live test clean deps deps-cli deps-registry deps-issuer deps-trust deps-log deps-gateway deps-live up down status

HELP_WIDTH = 18
help:                       ## Show this help.
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[1m%-$(HELP_WIDTH)s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

demo: deps                  ## Walk the ten STORY.md beats end to end.
	./scripts/demo.sh

demo-live: deps-live        ## Walk B1-B10 against the four LIVE sibling services.
	./scripts/demo-live.sh

test:                       ## Run the shell test harness (happy path + tamper tests).
	./tests/run_tests.sh

deps: deps-cli deps-registry deps-gateway ## Build every dependent binary (release).

deps-live: deps-cli deps-registry deps-issuer deps-trust deps-log deps-gateway ## Build every live-service binary (release).

# The sibling repos are developed in parallel; a rebuild can fail while
# a sibling is mid-edit. When that happens we fall back to the existing
# binary (the last known-good build) instead of blocking the demo.
deps-cli:                   ## Build unidpp-cli (release).
	@if cargo build --release --manifest-path ../unidpp-cli/Cargo.toml; then :; \
	elif [ -x ../unidpp-cli/target/release/unidpp ]; then \
		echo "warning: unidpp-cli rebuild failed (repo mid-edit); using existing binary"; \
	else echo "error: unidpp-cli has no binary and the build failed" >&2; exit 1; fi

deps-registry:              ## Build unidpp-registry (release).
	@if cargo build --release --manifest-path ../unidpp-registry/Cargo.toml; then :; \
	elif [ -x ../unidpp-registry/target/release/unidpp-registry ]; then \
		echo "warning: unidpp-registry rebuild failed (repo mid-edit); using existing binary"; \
	else echo "error: unidpp-registry has no binary and the build failed" >&2; exit 1; fi

deps-issuer:                ## Build unidpp-issuer when its source is present.
	@if [ -d ../unidpp-issuer/src ] && [ -n "$$(ls -A ../unidpp-issuer/src 2>/dev/null)" ]; then \
		if cargo build --release --manifest-path ../unidpp-issuer/Cargo.toml; then :; \
		elif [ -x ../unidpp-issuer/target/release/unidpp-issuer ]; then \
			echo "warning: unidpp-issuer rebuild failed (repo mid-edit); using existing binary"; \
		else echo "error: unidpp-issuer has no binary and the build failed" >&2; exit 1; fi; \
	else \
		echo "unidpp-issuer source not yet present ; skipping build."; \
	fi

deps-trust:                 ## Build unidpp-trust when its source is present.
	@if [ -d ../unidpp-trust/src ]; then \
		if cargo build --release --manifest-path ../unidpp-trust/Cargo.toml; then :; \
		elif [ -x ../unidpp-trust/target/release/unidpp-trust ]; then \
			echo "warning: unidpp-trust rebuild failed (repo mid-edit); using existing binary"; \
		else echo "error: unidpp-trust has no binary and the build failed" >&2; exit 1; fi; \
	else \
		echo "unidpp-trust source not present ; skipping build."; \
	fi

deps-log:                   ## Build unidpp-log when its source is present.
	@if [ -d ../unidpp-log/src ]; then \
		if cargo build --release --manifest-path ../unidpp-log/Cargo.toml; then :; \
		elif [ -x ../unidpp-log/target/release/unidpp-log ]; then \
			echo "warning: unidpp-log rebuild failed (repo mid-edit); using existing binary"; \
		else echo "error: unidpp-log has no binary and the build failed" >&2; exit 1; fi; \
	else \
		echo "unidpp-log source not present ; skipping build."; \
	fi

deps-gateway:               ## Build unidpp-gateway when its source is present.
	@if [ -d ../unidpp-gateway/src ] && [ -n "$$(ls -A ../unidpp-gateway/src 2>/dev/null)" ]; then \
		if cargo build --release --manifest-path ../unidpp-gateway/Cargo.toml; then :; \
		elif [ -x ../unidpp-gateway/target/release/unidpp-gateway ]; then \
			echo "warning: unidpp-gateway rebuild failed (repo mid-edit); using existing binary"; \
		else echo "error: unidpp-gateway has no binary and the build failed" >&2; exit 1; fi; \
	else \
		echo "unidpp-gateway source not yet present ; skipping build."; \
	fi

up:                         ## Start the registry in the background (compose).
	docker compose up -d registry

down:                       ## Stop the background services.
	docker compose down

status:                     ## Are the sibling services healthy?
	@curl -sf $$(printf '%s/healthz' "$${UNIDPP_REGISTRY_URL:-http://127.0.0.1:8098}") && echo "registry: ok" || echo "registry: down"

clean:                      ## Remove the build/ artifacts.
	rm -rf build/
