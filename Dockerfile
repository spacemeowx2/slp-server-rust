FROM ekidd/rust-musl-builder:1.42.0 AS BUILDER

ADD --chown=rust:rust . ./

RUN cargo build --release

FROM alpine:3.11

RUN apk add --no-cache tini

COPY --from=builder \
    /home/rust/src/target/x86_64-unknown-linux-musl/release/slp-server-rust \
    /usr/local/bin/

ENTRYPOINT ["/sbin/tini", "slp-server-rust"]
