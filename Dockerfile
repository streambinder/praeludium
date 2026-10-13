# ---- build ----
FROM golang:1.26-alpine AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOARCH=arm64 go build -trimpath -o /out/praeludium ./cmd/praeludium \
  && mkdir -p /out/data

# ---- runtime ----
FROM gcr.io/distroless/static:nonroot
COPY --from=build --chown=nonroot:nonroot /out/praeludium /praeludium
COPY --from=build --chown=nonroot:nonroot /out/data /data
USER nonroot
EXPOSE 8080
VOLUME /data
HEALTHCHECK --interval=30s --timeout=5s CMD ["/praeludium", "-healthcheck"]
ENTRYPOINT ["/praeludium"]
