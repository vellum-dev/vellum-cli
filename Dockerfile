# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM tonistiigi/xx AS xx

FROM --platform=$BUILDPLATFORM rust:alpine AS builder
RUN apk add --no-cache clang lld musl-dev git
COPY --from=xx / /

WORKDIR /src
COPY . .

ARG TARGETPLATFORM
RUN xx-cargo build --release --target-dir ./build && \
    xx-verify ./build/$(xx-cargo --print-target-triple)/release/vellum
