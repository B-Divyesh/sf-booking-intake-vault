FROM node:22-bookworm-slim AS web-builder
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY frontend ./frontend
RUN npm run build:web

FROM rust:1.88-bookworm AS rust-builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime
ARG BUILD_SHA=dev
RUN groupadd --system private-intake && useradd --system --gid private-intake --home-dir /app private-intake \
    && mkdir -p /app/frontend /data \
    && chown -R private-intake:private-intake /app /data
WORKDIR /app
COPY --from=rust-builder /app/target/release/booking-intake-vault /usr/local/bin/private-intake
COPY --from=web-builder /app/frontend/dist ./frontend/dist
USER private-intake
ENV PORT=8080 \
    APP_ENV=production \
    BUILD_SHA=${BUILD_SHA} \
    DATABASE_URL=sqlite:///data/booking-intake-vault.db?mode=rwc
EXPOSE 8080
VOLUME ["/data"]
CMD ["private-intake"]
