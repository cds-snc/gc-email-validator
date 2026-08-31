FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

ARG CARGO_LAMBDA_VERSION=1.9.2
ARG TERRAGRUNT_VERSION=1.1.4
ARG TERRAFORM_VERSION=1.15.8
ARG ZIG_VERSION=0.15.2
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl jq unzip xz-utils zip \
    && rm -rf /var/lib/apt/lists/*

RUN architecture="$(dpkg --print-architecture)" \
    && case "${architecture}" in \
        amd64) terraform_arch="amd64" ;; \
        arm64) terraform_arch="arm64" ;; \
        *) echo "Unsupported architecture: ${architecture}" >&2; exit 1 ;; \
       esac \
    && terraform_archive="terraform_${TERRAFORM_VERSION}_linux_${terraform_arch}.zip" \
    && curl --fail --location --show-error --silent \
       "https://releases.hashicorp.com/terraform/${TERRAFORM_VERSION}/${terraform_archive}" \
       --output "/tmp/${terraform_archive}" \
    && curl --fail --location --show-error --silent \
       "https://releases.hashicorp.com/terraform/${TERRAFORM_VERSION}/terraform_${TERRAFORM_VERSION}_SHA256SUMS" \
       --output /tmp/terraform_SHA256SUMS \
    && cd /tmp \
    && grep "terraform_${TERRAFORM_VERSION}_linux_${terraform_arch}.zip" terraform_SHA256SUMS | sha256sum --check \
    && unzip "${terraform_archive}" -d /usr/local/bin \
    && rm "${terraform_archive}" terraform_SHA256SUMS

RUN architecture="$(dpkg --print-architecture)" \
    && case "${architecture}" in \
        amd64) \
          terragrunt_arch="amd64"; \
          terragrunt_sha256="a2640da8455fa5f3671167e6373832b0907b9dc972dd01c2093cc7808934e158" \
          ;; \
        arm64) \
          terragrunt_arch="arm64"; \
          terragrunt_sha256="c65d1897446590ebb3c695835cc956c12c5374a9add8312517c83c9fd7a1c06b" \
          ;; \
        *) echo "Unsupported architecture: ${architecture}" >&2; exit 1 ;; \
       esac \
    && terragrunt_binary="terragrunt_linux_${terragrunt_arch}" \
    && curl --fail --location --show-error --silent \
       "https://github.com/gruntwork-io/terragrunt/releases/download/v${TERRAGRUNT_VERSION}/${terragrunt_binary}" \
       --output "/tmp/${terragrunt_binary}" \
    && echo "${terragrunt_sha256}  /tmp/${terragrunt_binary}" | sha256sum --check \
    && install -m 0755 "/tmp/${terragrunt_binary}" /usr/local/bin/terragrunt \
    && rm "/tmp/${terragrunt_binary}"

RUN architecture="$(dpkg --print-architecture)" \
    && case "${architecture}" in \
        amd64) \
          zig_arch="x86_64"; \
          zig_sha256="02aa270f183da276e5b5920b1dac44a63f1a49e55050ebde3aecc9eb82f93239" \
          ;; \
        arm64) \
          zig_arch="aarch64"; \
          zig_sha256="958ed7d1e00d0ea76590d27666efbf7a932281b3d7ba0c6b01b0ff26498f667f" \
          ;; \
        *) echo "Unsupported architecture: ${architecture}" >&2; exit 1 ;; \
       esac \
    && zig_archive="zig-${zig_arch}-linux-${ZIG_VERSION}.tar.xz" \
    && curl --fail --location --show-error --silent \
       "https://ziglang.org/download/${ZIG_VERSION}/${zig_archive}" \
       --output "/tmp/${zig_archive}" \
    && echo "${zig_sha256}  /tmp/${zig_archive}" | sha256sum --check \
    && mkdir /opt/zig \
    && tar --extract --xz --file "/tmp/${zig_archive}" --directory /opt/zig --strip-components=1 \
    && ln --symbolic /opt/zig/zig /usr/local/bin/zig \
    && rm "/tmp/${zig_archive}"

RUN architecture="$(dpkg --print-architecture)" \
    && case "${architecture}" in \
        amd64) \
          cargo_lambda_arch="x86_64"; \
          cargo_lambda_sha256="b5c258c424ebaef39541c3641f33c51137ee6c426016f365628fedb2c737271d" \
          ;; \
        arm64) \
          cargo_lambda_arch="aarch64"; \
          cargo_lambda_sha256="c3fd0a352bb439e4e84a09e5361ea333f7cdc58b0894105e93a33c43b867bc2d" \
          ;; \
        *) echo "Unsupported architecture: ${architecture}" >&2; exit 1 ;; \
       esac \
    && archive="cargo-lambda-v${CARGO_LAMBDA_VERSION}.${cargo_lambda_arch}-unknown-linux-musl.tar.gz" \
    && curl --fail --location --show-error --silent \
       "https://github.com/cargo-lambda/cargo-lambda/releases/download/v${CARGO_LAMBDA_VERSION}/${archive}" \
       --output "/tmp/${archive}" \
    && echo "${cargo_lambda_sha256}  /tmp/${archive}" | sha256sum --check \
    && tar --extract --gzip --file "/tmp/${archive}" --directory /usr/local/bin cargo-lambda \
    && rm "/tmp/${archive}"
