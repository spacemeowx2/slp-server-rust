FROM clux/muslrust:stable AS BUILDER

COPY . ./

RUN cargo build --release

FROM alpine:3.11

COPY --from=builder \
    /volume/target/x86_64-unknown-linux-musl/release/slp-server-rust \
    /usr/local/bin/

ENTRYPOINT ["slp-server-rust"]
