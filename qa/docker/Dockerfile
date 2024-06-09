FROM rust AS builder
ARG PROTOC_VERSION
ENV PROTOC_VERSION=${PROTOC_VERSION:-27.0}
RUN --mount=type=bind,from=code,target=/mnt <<EOT
  set -eu
  curl -sLO https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-x86_64.zip
  unzip protoc-${PROTOC_VERSION}-linux-x86_64.zip bin/protoc -d /usr/local
  cargo install --path /mnt/crates/cli --target-dir /tmp
EOT

FROM debian:bookworm-slim
COPY --from=builder /usr/local/cargo/bin/malachite-cli /usr/local/bin/malachite-cli
ENTRYPOINT ["malachite-cli"]
