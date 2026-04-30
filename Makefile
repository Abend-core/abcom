..PHONY: all build install uninstall run clean test test-verbose test-module test-watch

export PATH := $(HOME)/.cargo/bin:$(PATH)
CARGO := cargo

BINARY_NAME := abcom
INSTALL_DIR := $(HOME)/.local/bin
SERVICE_DIR := $(HOME)/.config/systemd/user
SERVICE_NAME := abcom.service
SYSTEMCTL := $(shell command -v systemctl 2>/dev/null || true)
LOGINCTL := $(shell command -v loginctl 2>/dev/null || true)

all: build

## Compile en mode développement
build:
	$(CARGO) build

## Compile en mode release (optimisé)
release:
	$(CARGO) build --release

## Lance l'application localement
run:
	$(CARGO) run --release

## Lance l'application sur Windows depuis WSL
run-windows:
	$(CARGO) build --release --target x86_64-pc-windows-gnu
	@mkdir -p /mnt/c/Users/$(USER)/AppData/Local/abcom
	@mkdir -p /mnt/c/Users/$(USER)/AppData/Local/abcom/assets
	@cp -f target/x86_64-pc-windows-gnu/release/abcom.exe /mnt/c/Users/$(USER)/AppData/Local/abcom/abcom_new.exe 2>/dev/null || true
	@cp -f assets/app_icon.jpg /mnt/c/Users/$(USER)/AppData/Local/abcom/assets/ 2>/dev/null || true
	@/mnt/c/Windows/System32/cmd.exe /c start "" "C:\Users\$(USER)\AppData\Local\abcom\abcom_new.exe" $(USER)

## Installe le binaire + active le service systemd + raccourci menu
install: release
	@mkdir -p $(INSTALL_DIR)
	@if [ -n "$(SYSTEMCTL)" ]; then \
		systemctl --user stop $(SERVICE_NAME) 2>/dev/null || true; \
	fi
	cp target/release/$(BINARY_NAME) $(INSTALL_DIR)/$(BINARY_NAME)
	chmod +x $(INSTALL_DIR)/$(BINARY_NAME)
	@mkdir -p $(HOME)/.local/share/applications
	cp contrib/abcom.desktop $(HOME)/.local/share/applications/abcom.desktop
	@mkdir -p $(HOME)/.local/share/$(BINARY_NAME)
	@if [ -n "$(SYSTEMCTL)" ]; then \
		mkdir -p $(SERVICE_DIR); \
		cp contrib/$(SERVICE_NAME) $(SERVICE_DIR)/$(SERVICE_NAME); \
		if [ -n "$(LOGINCTL)" ]; then loginctl enable-linger $(USER) 2>/dev/null || true; fi; \
		systemctl --user daemon-reload; \
		systemctl --user enable --now $(SERVICE_NAME); \
		printf "\n✓ %s installé dans %s\n✓ Raccourci menu créé (Applications → Abcom)\n✓ Service systemd activé (démarrage automatique)\n" "$(BINARY_NAME)" "$(INSTALL_DIR)"; \
	else \
		printf "\n✓ %s installé dans %s\n✓ Raccourci menu créé (Applications → Abcom)\n⚠️  systemd non trouvé : installation limitée au binaire et au raccourci\n" "$(BINARY_NAME)" "$(INSTALL_DIR)"; \
	fi

## Prépare le binaire pour distribution (copie dans /tmp)
deploy-bin: release
	@cp target/release/$(BINARY_NAME) /tmp/$(BINARY_NAME)
	@cp scripts/abcom-install.sh /tmp/abcom-install.sh
	@chmod +x /tmp/abcom-install.sh
	@echo ""
	@echo "📦 Binaire prêt pour distribution:"
	@echo "   Fichier: /tmp/$(BINARY_NAME)"
	@echo "   Script:  /tmp/abcom-install.sh"
	@echo ""
	@echo "💾 Pour partager:"
	@echo "   zip /tmp/abcom-deploy.zip /tmp/abcom /tmp/abcom-install.sh"
	@echo "   # Puis envoie le ZIP à tes copains !"

## Installe depuis un binaire pré-compilé (sans compiler)
install-bin: 
	@if [ ! -f "$(CURDIR)/target/release/$(BINARY_NAME)" ]; then \
		echo "❌ Erreur: binaire non trouvé !"; \
		echo "   Lance d'abord: make deploy-bin"; \
		exit 1; \
	fi
	bash scripts/abcom-install.sh $(CURDIR)/target/release/$(BINARY_NAME)

## Désinstalle le binaire et le service
uninstall:
	@if [ -n "$(SYSTEMCTL)" ]; then \
		systemctl --user stop $(SERVICE_NAME) 2>/dev/null || true; \
		systemctl --user disable $(SERVICE_NAME) 2>/dev/null || true; \
		systemctl --user daemon-reload; \
	fi
	rm -f $(SERVICE_DIR)/$(SERVICE_NAME)
	rm -f $(INSTALL_DIR)/$(BINARY_NAME)
	@echo "✓ $(BINARY_NAME) désinstallé"

## Supprime les artefacts de compilation
clean:
	cargo clean

## Lance tous les tests unitaires
test:
	$(CARGO) test

## Tests avec sortie complète (println! visibles)
test-verbose:
	$(CARGO) test -- --nocapture

## Tests d'un module spécifique  ex: make test-module M=app::peers
test-module:
	@test -n "$(M)" || (echo "Usage: make test-module M=app::peers" && exit 1)
	$(CARGO) test $(M) -- --nocapture

## Tests en mode watch (cargo-watch requis: cargo install cargo-watch)
test-watch:
	@command -v cargo-watch >/dev/null 2>&1 || (echo "❌  cargo-watch non installé — lance: cargo install cargo-watch" && exit 1)
	cargo watch -x test
