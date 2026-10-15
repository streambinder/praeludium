# ---- build ----
FROM rust:1-alpine3.24 AS build
WORKDIR /src
RUN apk add --no-cache musl-dev=1.2.6-r2
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY templates ./templates
COPY static ./static
COPY locales ./locales
RUN cargo build --release --locked \
  && mkdir -p /out/data \
  && cp target/release/praeludium /out/praeludium

# ---- runtime ----
FROM gcr.io/distroless/static:nonroot
COPY --from=build --chown=nonroot:nonroot /out/praeludium /praeludium
COPY --from=build --chown=nonroot:nonroot /out/data /data
USER nonroot
EXPOSE 8080
VOLUME /data
HEALTHCHECK --interval=30s --timeout=5s CMD ["/praeludium", "-healthcheck"]
ENTRYPOINT ["/praeludium"]
