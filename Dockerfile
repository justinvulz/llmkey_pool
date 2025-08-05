# Dockerfile

# --- Stage 1: Build the application ---
FROM rust:1.88-alpine AS builder

WORKDIR /app

# Install openssl-dev for reqwest (important for HTTPS)
RUN apk add --no-cache  ca-certificates build-base 

# Copy Cargo.toml and Cargo.lock first to leverage Docker cache for dependencies
COPY Cargo.toml Cargo.lock ./

# Create a dummy src/main.rs to build dependencies and cache them
RUN mkdir -p src && echo "fn main() {println!(\"hello\");}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl
# Remove the dummy src/main.rs
RUN rm -rf src

# Copy the actual source code
COPY src ./src

# Build the final application
RUN cargo build --release --target x86_64-unknown-linux-musl

RUN pwd
RUN ls -la
RUN ls ./target -la
RUN ls ./target/x86_64-unknown-linux-musl -la

# --- Stage 2: Create the final lean image ---
FROM alpine

# Set working directory inside the container
WORKDIR /app

# Install necessary runtime dependencies (e.g., for TLS/SSL certificates)
RUN apk add --no-cache ca-certificates curl


# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/llmkey_pool .


# Expose the port your application listens on (as defined in config.toml)
EXPOSE 3030

# (Optional) Create a non-root user for security
# ARG UID=1000
# ARG GID=1000
# RUN groupadd --gid $GID appuser && useradd --uid $UID --gid $GID -m appuser
# USER appuser

# Command to run the application
CMD ["./llmkey_pool"]
