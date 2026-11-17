# Build: docker build -t tuv-hub .
# Run:   docker run -p 8080:8080 -v %cd%\config.toml:/app/config.toml:ro tuv-hub
# Build and runtime images must share the Debian release (both 12 "bookworm"): a binary built
# against a newer glibc refuses to start on an older one.
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY web ./web
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=build /src/target/release/tuv-hub /app/tuv-hub
EXPOSE 8080
# Listen on all container interfaces; Docker's -p decides what is reachable from outside.
ENTRYPOINT ["/app/tuv-hub", "--config", "/app/config.toml", "--bind", "0.0.0.0:8080"]
