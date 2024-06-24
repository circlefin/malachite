# Add the "code" build context to access local source code during build.
#  docker build . --build-context code=$MALACHITE_DIR/code
FROM rust AS builder
RUN apt-get update && apt-get install -y protobuf-compiler
RUN --mount=type=bind,from=code,target=/mnt cargo install --path /mnt/crates/cli --target-dir /tmp

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y iproute2
COPY --from=builder /usr/local/cargo/bin/malachite-cli /usr/local/bin/malachite-cli
ENTRYPOINT ["malachite-cli"]
