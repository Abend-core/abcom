.PHONY: all build release install uninstall run run2 rung run-multi run-windows help clean check test testv test-verbose test-module test-watch deploy-bin install-bin

export PATH := $(HOME)/.cargo/bin:$(PATH)
CARGO := cargo

BINARY_NAME := abcom
INSTALL_DIR := $(HOME)/.local/bin
SERVICE_DIR := $(HOME)/.config/systemd/user
SERVICE_NAME := abcom.service
# Évalué à chaque invocation de make, quelle que soit la cible : sous Windows,
# make passe par cmd.exe, qui ne connaît ni `command` ni `true` et affiche deux
# erreurs avant chaque `make run`. Les cibles qui s'en servent (install,
# uninstall) sont de toute façon spécifiques à systemd.
ifeq ($(OS),Windows_NT)
SYSTEMCTL :=
else
SYSTEMCTL := $(shell command -v systemctl 2>/dev/null || true)
endif

all: build

## Vérifie le formatage, Clippy et tous les tests
check:
	$(CARGO) fmt --all --check
	$(CARGO) clippy --all-targets --all-features --locked -- -D warnings
	$(CARGO) test --all-features --locked

## Compile en mode développement
build:
	$(CARGO) build

## Compile en mode release (optimisé)
release:
	$(CARGO) build --release

## Lance l'application localement
run:
	$(CARGO) run --release

## Lance 2 fenêtres locales pour tester la connexion P2P (chat 1-à-1)
run2:
	@bash scripts/run-multi.sh 2

## Lance 3 fenêtres locales pour tester les groupes
rung:
	@bash scripts/run-multi.sh 3

## Lance N fenêtres locales (ex: make run-multi N=4)
run-multi:
	@bash scripts/run-multi.sh $(or $(N),2)

## Lance l'application sur Windows depuis WSL
run-windows:
	$(CARGO) build --release --target x86_64-pc-windows-gnu
	@mkdir -p /mnt/c/Users/$(USER)/AppData/Local/abcom
	@mkdir -p /mnt/c/Users/$(USER)/AppData/Local/abcom/assets
	@cp -f target/x86_64-pc-windows-gnu/release/abcom.exe /mnt/c/Users/$(USER)/AppData/Local/abcom/abcom_new.exe 2>/dev/null || true
	@cp -f assets/app_icon.jpg /mnt/c/Users/$(USER)/AppData/Local/abcom/assets/ 2>/dev/null || true
	@cd /mnt/c/Users/$(USER)/AppData/Local/abcom && /mnt/c/Windows/System32/cmd.exe /c start "" "C:\Users\$(USER)\AppData\Local\abcom\abcom_new.exe" $(USER)

## Affiche l'aide des cibles du Makefile
help:
	@printf "\nTargets disponibles:\n"
	@printf "  make build            - compile en mode développement\n"
	@printf "  make release          - compile en mode release optimisé\n"
	@printf "  make run              - lance l'application localement\n"
	@printf "  make run2             - lance 2 fenêtres pour tester la connexion P2P\n"
	@printf "  make rung             - lance 3 fenêtres pour tester les groupes\n"
	@printf "  make run-multi N=4    - lance N fenêtres locales\n"
	@printf "  make run-windows      - compile pour Windows depuis WSL et lance l'appli\n"
	@printf "  make install          - installe binaire + raccourci + service systemd\n"
	@printf "  make uninstall        - désinstalle le binaire et le service\n"
	@printf "  make clean            - supprime les artefacts de compilation\n"
	@printf "  make test             - lance tous les tests\n"
	@printf "  make test-verbose     - lance les tests avec --nocapture\n"
	@printf "  make test-module M=.. - lance un module de test spécifique\n"
	@printf "  make test-watch       - lance les tests en mode watch\n\n"

## Installe le binaire + active le service systemd + raccourci menu
install: release
	@mkdir -p $(INSTALL_DIR)
	cp target/release/$(BINARY_NAME) $(INSTALL_DIR)/$(BINARY_NAME)
	chmod +x $(INSTALL_DIR)/$(BINARY_NAME)
	@mkdir -p $(HOME)/.local/share/applications
	cp scripts/abcom.desktop $(HOME)/.local/share/applications/abcom.desktop
	@mkdir -p $(HOME)/.local/share/$(BINARY_NAME)
	@# Reste d'une installation antérieure : un service systemd doublait le
	@# démarrage XDG de l'application et lançait une seconde instance.
	@if [ -n "$(SYSTEMCTL)" ]; then \
		systemctl --user disable --now $(SERVICE_NAME) 2>/dev/null || true; \
		rm -f $(SERVICE_DIR)/$(SERVICE_NAME); \
		systemctl --user daemon-reload 2>/dev/null || true; \
	fi
	@printf "\n✓ %s installé dans %s\n✓ Raccourci menu créé (Applications → Abcom)\n✓ Démarrage à l'ouverture de session : réglable dans Paramètres → Général\n" "$(BINARY_NAME)" "$(INSTALL_DIR)"

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
testv:
	$(CARGO) test -- --nocapture

test-verbose: testv

## Tests d'un module spécifique  ex: make test-module M=app::peers
test-module:
	@test -n "$(M)" || (echo "Usage: make test-module M=app::peers" && exit 1)
	$(CARGO) test $(M) -- --nocapture

## Tests en mode watch (cargo-watch requis: cargo install cargo-watch)
test-watch:
	@command -v cargo-watch >/dev/null 2>&1 || (echo "❌  cargo-watch non installé — lance: cargo install cargo-watch" && exit 1)
	cargo watch -x test
