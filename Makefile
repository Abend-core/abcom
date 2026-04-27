.PHONY: all build install uninstall run clean

BINARY_NAME := abcom
INSTALL_DIR := $(HOME)/.local/bin
SERVICE_DIR := $(HOME)/.config/systemd/user
SERVICE_NAME := abcom.service

all: build

## Compile en mode développement
build:
	cargo build

## Compile en mode release (optimisé)
release:
	cargo build --release

## Lance directement (développement)
run:
	cargo run -- $(USER)

## Installe le binaire + active le service systemd
install: release
	@mkdir -p $(INSTALL_DIR)
	cp target/release/$(BINARY_NAME) $(INSTALL_DIR)/$(BINARY_NAME)
	chmod +x $(INSTALL_DIR)/$(BINARY_NAME)
	@mkdir -p $(SERVICE_DIR)
	cp contrib/$(SERVICE_NAME) $(SERVICE_DIR)/$(SERVICE_NAME)
	loginctl enable-linger $(USER) 2>/dev/null || true
	systemctl --user daemon-reload
	systemctl --user enable --now $(SERVICE_NAME)
	@echo ""
	@echo "✓ $(BINARY_NAME) installé dans $(INSTALL_DIR)"
	@echo "✓ Service systemd activé (démarrage automatique)"

## Désinstalle le binaire et le service
uninstall:
	systemctl --user stop $(SERVICE_NAME) 2>/dev/null || true
	systemctl --user disable $(SERVICE_NAME) 2>/dev/null || true
	rm -f $(SERVICE_DIR)/$(SERVICE_NAME)
	systemctl --user daemon-reload
	rm -f $(INSTALL_DIR)/$(BINARY_NAME)
	@echo "✓ $(BINARY_NAME) désinstallé"

## Supprime les artefacts de compilation
clean:
	cargo clean
