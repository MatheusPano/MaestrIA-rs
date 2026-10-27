.DEFAULT_GOAL := help
SHELL := /bin/bash

RUST_VERSION ?= 1.93.0
export RUST_VERSION

CARGO    ?= cargo
DIST     := packaging/linux/dist
MAC_DIST := packaging/macos/dist
RAW_VERSION := $(shell sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -1)
REVISION    := $(shell sed -n '/^\[package.metadata.deb\]/,/^\[/s/^revision *= *"\([^"]*\)".*/\1/p' crates/mx-app/Cargo.toml | head -1)
VERSION     := $(RAW_VERSION)-$(or $(REVISION),1)
DEB         := $(DIST)/maestria_$(VERSION)_amd64.deb

.PHONY: help deb deb-only deb-test dmg release analyze clippy test bench bump clean tag version

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
	@./packaging/linux/build-deb.sh

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
	@git push origin "v$(RAW_VERSION)"
	@echo
	@echo "v$(RAW_VERSION) empurrada. o .deb e o .dmg aparecem em"
	@echo "https://github.com/MatheusPano/MaestrIA/releases/tag/v$(RAW_VERSION)"

bump:
ifndef V
	@echo "uso: make bump V=1.0.1"; exit 1
endif
	@perl -0pi -e 's/(\[workspace\.package\]\n(?:[^\[]*?\n)?version\s*=\s*)"[^"]*"/$${1}"$(V)"/' Cargo.toml
	@echo "Cargo.toml: $$(sed -n '/^\[workspace.package\]/,/^\[/s/^version.*/&/p' Cargo.toml)"

clean:
	@rm -rf $(DIST) $(MAC_DIST)
	@echo "$(DIST) e $(MAC_DIST) limpos"
