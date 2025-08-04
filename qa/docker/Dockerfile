# Add the "code" build context to access local source code during build.
#  docker build . --build-context code=$MALACHITE_DIR/code
FROM rust AS builder
RUN apt-get update && apt-get install -y protobuf-compiler
WORKDIR /mnt
RUN --mount=type=bind,from=code,target=/mnt,rw cargo build --release --target-dir /tmp

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y iproute2
COPY --from=builder /tmp/release/informalsystems-malachitebft-starknet-app /usr/local/bin/malachite-cli
COPY --chmod=755 entrypoint.sh /usr/bin/entrypoint.sh
ENTRYPOINT ["/usr/bin/entrypoint.sh"]
