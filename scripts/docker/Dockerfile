FROM rust:1.95

WORKDIR /app

# Installer les dépendances pour egui/eframe
RUN apt-get update && apt-get install -y \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxkbcommon-dev \
    libasound2-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    libxdo-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY assets ./assets

RUN cargo build --release --locked

ENTRYPOINT ["target/release/abcom"]
CMD ["alice"]
