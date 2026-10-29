.DEFAULT_GOAL := help
SHELL := /bin/bash

RUST_VERSION ?= 1.93.0
export RUST_VERSION

CARGO    ?= cargo
REMOTE   ?= upstream
REPO     ?= MatheusPano/MaestrIA-rs
DIST     := packaging/linux/dist
MAC_DIST := packaging/macos/dist
RAW_VERSION := $(shell sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -1)
REVISION    := $(shell sed -n '/^\[package.metadata.deb\]/,/^\[/s/^revision *= *"\([^"]*\)".*/\1/p' crates/mx-app/Cargo.toml | head -1)
VERSION     := $(RAW_VERSION)-$(or $(REVISION),1)
DEB         := $(DIST)/maestria-rs_$(VERSION)_amd64.deb

.PHONY: help deb deb-only deb-test dmg release publish analyze clippy test bench bump clean tag version

help:
	@echo "maestria $(VERSION)"
	@echo
	@echo "  make deb        clippy e gera o .deb em $(DIST)/"
	@echo "  make deb-only   gera o .deb sem o clippy antes"
	@echo "  make deb-test   instala o .deb numa ubuntu:22.04 limpa e abre o app"
	@echo "  make dmg        gera o .dmg universal do Mac em $(MAC_DIST)/ (só no macOS)"
	@echo "  make release    clippy, testes, .deb e smoke test -- o caminho completo"
	@echo
	@echo "  make tag        publica a v$(RAW_VERSION) no GitHub (.deb e .dmg saem do CI)"
	@echo "  make publish    gera o .deb nesta máquina e sobe na release v$(RAW_VERSION) com o SHA256SUMS (cria se faltar)"
	@echo
	@echo "  make clippy     cargo clippy --workspace -D warnings"
	@echo "  make test       cargo test --workspace"
	@echo "  make bench      cargo bench --workspace"
	@echo "  make bump V=1.0.1   sobe a versão do workspace (o apt só atualiza se ela mudar)"
	@echo "  make clean      apaga os pacotes gerados"
	@echo
	@echo "  o .deb precisa de cargo-deb (cargo install cargo-deb); o .dmg, de cargo-bundle."

version:
	@echo $(VERSION)

analyze: clippy

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

test:
	$(CARGO) test --workspace

bench:
	$(CARGO) bench --workspace

deb: clippy deb-only

deb-only:
	@./packaging/linux/build-deb.sh --variant rs

deb-test:
	@./packaging/linux/smoke-test.sh

dmg:
	@./packaging/macos/make-dmg.sh

release: clippy test deb-only deb-test
	@echo
	@echo "pronto: $(DEB)"

tag:
	@git diff --quiet || { echo "há alterações não commitadas -- commite antes de taguear."; exit 1; }
	@git tag "v$(RAW_VERSION)"
	@git push $(REMOTE) "v$(RAW_VERSION)"
	@echo
	@echo "v$(RAW_VERSION) empurrada. o .deb e o .dmg aparecem em"
	@echo "https://github.com/MatheusPano/MaestrIA-rs/releases/tag/v$(RAW_VERSION)"

bump:
ifndef V
	@echo "uso: make bump V=1.0.1"; exit 1
endif
	@perl -0pi -e 's/(\[workspace\.package\]\n(?:[^\[]*?\n)?version\s*=\s*)"[^"]*"/$${1}"$(V)"/' Cargo.toml
	@echo "Cargo.toml: $$(sed -n '/^\[workspace.package\]/,/^\[/s/^version.*/&/p' Cargo.toml)"

clean:
	@rm -rf $(DIST) $(MAC_DIST)
	@echo "$(DIST) e $(MAC_DIST) limpos"

publish:
	@git diff --quiet || { echo "há alterações não commitadas -- commite antes de publicar."; exit 1; }
	@NATIVE=1 MAESTRIA_VERSION=$(RAW_VERSION) ./packaging/linux/build-deb.sh --variant rs
	@git rev-parse -q --verify "refs/tags/v$(RAW_VERSION)" >/dev/null || git tag "v$(RAW_VERSION)"
	@git push $(REMOTE) "v$(RAW_VERSION)" 2>/dev/null || true
	@rm -rf "$(DIST)/published" && mkdir -p "$(DIST)/published"
	@gh release download "v$(RAW_VERSION)" -R $(REPO) -p SHA256SUMS -D "$(DIST)/published" 2>/dev/null || true
	@{ grep -v " \*\?$(notdir $(DEB))$$" "$(DIST)/published/SHA256SUMS" 2>/dev/null; cd $(DIST) && sha256sum "$(notdir $(DEB))"; } > "$(DIST)/SHA256SUMS"
	@cat "$(DIST)/SHA256SUMS"
	@gh release view "v$(RAW_VERSION)" -R $(REPO) >/dev/null 2>&1 \
	  && gh release upload "v$(RAW_VERSION)" "$(DEB)" "$(DIST)/SHA256SUMS" -R $(REPO) --clobber \
	  || gh release create "v$(RAW_VERSION)" "$(DEB)" "$(DIST)/SHA256SUMS" -R $(REPO) --title "maestria-rs $(RAW_VERSION)" --notes "$$(sed "s/{TAG}/v$(RAW_VERSION)/g" packaging/NOTAS-DA-RELEASE.md)"
	@echo "https://github.com/$(REPO)/releases/tag/v$(RAW_VERSION)"
