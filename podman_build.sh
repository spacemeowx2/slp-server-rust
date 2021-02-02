#!/bin/bash
# Simple conversion from the provided Dockerfile to a shell script using buildah for environment
# without Docker

set -ueo  pipefail
_r="$(realpath $0)"
REPO="${_r%/*}"
OUT="${REPO}/out"

## CONFIG
# The containerized build system to use
SRC_IMAGE='ekidd/rust-musl-builder:1.49.0'
# The tag to use for the resulting image
TAG=latest
# Whether to keep the resulting binary or not
KEEP_RESULT=
## END CONFIG

mkdir -pv "$OUT"
CTR=$(buildah from --pull --userns host --volume "${OUT}:/out" "$SRC_IMAGE")

buildah add --chown rust:rust "$CTR" "$REPO" ./
buildah run -- "$CTR" cargo build --release
buildah run --user root -- "$CTR" cp /home/rust/src/target/x86_64-unknown-linux-musl/release/slp-server-rust /out

CTR=$(buildah from --volume "${OUT}:/out" scratch)
buildah add "$CTR" "${OUT}/slp-server-rust" /

buildah config \
  --entrypoint '["/slp-server-rust"]' \
  --port 11451 \
  "$CTR"

buildah commit "$CTR" "slp-server-rust:${TAG}"

[ -z "$KEEP_RESULT" ] && rm -r "$OUT"
