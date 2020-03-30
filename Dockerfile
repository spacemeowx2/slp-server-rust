FROM ekidd/rust-musl-builder:1.42.0 AS BUILDER

ADD --chown=rust:rust . ./

RUN cargo build --release

FROM alpine:3.11

COPY --from=builder \
    /home/rust/src/target/x86_64-unknown-linux-musl/release/slp-server-rust \
    /usr/local/bin/

CMD ["slp-server-rust"]
