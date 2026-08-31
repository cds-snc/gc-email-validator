CHECK_TG_ENV = TG_AWS_ACCOUNT_ID=000000000000

.PHONY: backend-bootstrap build check clean compile-data deploy-package fmt infrastructure-check refresh test

fmt:
	cargo fmt --all
	terraform -chdir=aws fmt -recursive
	terragrunt hcl format --working-dir terragrunt

test:
	cargo test --workspace --all-targets

check:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-targets

refresh:
	./scripts/refresh-data.sh

compile-data:
	cargo run --locked -p domain-compiler
	rustfmt crates/api/src/generated_domains.rs

build:
	cargo build --workspace

deploy-package:
	cargo lambda build --release --arm64 --package gc-email-validator --output-format zip
	mkdir -p build
	cp target/lambda/gc-email-validator/bootstrap.zip build/lambda.zip

# Run manually with authenticated, privileged AWS credentials. Never run this
# target from CI: it establishes the remote state backend used by CI.
backend-bootstrap:
	TG_WORKING_DIR=terragrunt/lambda terragrunt backend bootstrap

infrastructure-check:
	terraform -chdir=aws fmt -check -recursive
	terraform -chdir=aws/lambda init -backend=false
	terraform -chdir=aws/lambda validate
	terragrunt hcl format --check --working-dir terragrunt
	$(CHECK_TG_ENV) terragrunt hcl validate --working-dir terragrunt
	$(CHECK_TG_ENV) TG_WORKING_DIR=terragrunt/lambda terragrunt run init --non-interactive -- -backend=false
	$(CHECK_TG_ENV) TG_WORKING_DIR=terragrunt/lambda terragrunt run validate --non-interactive --no-auto-init

clean:
	cargo clean
