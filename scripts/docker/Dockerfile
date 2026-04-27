FROM rust:1.95

WORKDIR /app

# Installer les dépendances pour egui/eframe
RUN apt-get update && apt-get install -y \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxkbcommon-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
COPY src ./src

RUN cargo build --release 2>&1 | tail -20

ENTRYPOINT ["target/release/abcom"]
CMD ["alice"]
