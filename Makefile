.PHONY: setup check check-small check-medium fmt fixtures corpus help

setup: ## Enable the git hooks in .githooks
	git config core.hooksPath .githooks

check: check-small check-medium ## Run every check of the task transition

check-small: ## Lint, custom lints, dependencies and small tests
	scripts/check-small.sh
	scripts/judge-small-test-coverage.sh out/small.lcov

check-medium: ## Fixtures, fixtures against the oracle, corpus regressions
	scripts/test-fixtures.sh
	scripts/check-fixtures-oracle.sh
	scripts/corpus-diff.sh dev

fmt: ## Apply rustfmt
	cargo fmt --all

fixtures: ## Run the fixtures
	scripts/test-fixtures.sh

corpus: ## Measure the dev corpus against the oracle
	scripts/corpus-diff.sh dev

help: ## Display this help screen
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "%-14s %s\n", $$1, $$2}'
