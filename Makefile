
# Use bash for recipes and fail fast
SHELL := /bin/bash
.ONESHELL:
.SHELLFLAGS := -eu -o pipefail -c

# Tools & flags
CARGO    ?= cargo
COMPOSE  ?= docker compose
FEATURES ?= --all-features
WORKSPACE ?= --workspace
LOCKED   ?= --locked

.DEFAULT_GOAL := ci

.PHONY: ci build test fmt clippy lint run docker-build docker-up docker-down compose-test clean help

## Run full pipeline: fmt → clippy → build → test → docker compose test
ci: fmt clippy build audit test compose-test

audit:
	${CARGO} audit

## Build all workspace crates
build:
	$(CARGO) build $(WORKSPACE) $(FEATURES) $(LOCKED)

## Format code
fmt:
	$(CARGO) fmt --all

## Clippy lints (treat warnings as errors)
clippy:
	$(CARGO) clippy --all-targets $(FEATURES) -- -D warnings

## Host tests
test:
	$(CARGO) test $(WORKSPACE) $(FEATURES) $(LOCKED) -- --nocapture

## Build images
docker-build:
	$(COMPOSE) build

## Bring up stack (detached)
docker-up:
	$(COMPOSE) up -d

## Bring stack down & clean
docker-down:
	$(COMPOSE) down --volumes --remove-orphans

## Run compose-driven tests (expects a `tests` service)
compose-test:
	$(COMPOSE) build
	trap '$(COMPOSE) down --volumes --remove-orphans' EXIT
	$(COMPOSE) up --abort-on-container-exit --exit-code-from tests

## Dev run of your two processes (ctrl+c to stop)
run:
	# Adjust paths/ports as needed
	RUST_LOG=info $(CARGO) run -p appgate-auth -- --uds /run/appgate/pdp.sock --policy config/policy/foundry.toml & pid1=$$!
	RUST_LOG=info $(CARGO) run -p appgate-mod-http -- --bind 0.0.0.0:8080 --pdp-uds /run/appgate/pdp.sock --upstream http://127.0.0.1:3000 & pid2=$$!
	trap 'kill $$pid1 $$pid2 2>/dev/null || true' EXIT
	wait -n

## Clean target dir (useful if features or build deps changed)
clean:
	$(CARGO) clean

## Show this help
help:
	@awk 'BEGIN{FS":.*##"; printf "Targets:\n"} /^[a-zA-Z0-9_%-]+:.*##/{printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)
