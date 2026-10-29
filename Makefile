.PHONY: all release debug clean install uninstall aur deb rpm tar debug-deb debug-rpm debug-tar package debug-package test

CARGO ?= cargo
PREFIX ?= /usr/local
DESTDIR ?=
BUILD_DIR := build

VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
UNAME_M := $(shell uname -m)
ARCH := $(shell if [ "$(UNAME_M)" = "aarch64" ] || [ "$(UNAME_M)" = "arm64" ]; then echo aarch64; else echo x86_64; fi)
DEB_ARCH := $(shell if [ "$(ARCH)" = "aarch64" ]; then echo arm64; else echo amd64; fi)
LIBC := $(shell if ldd --version 2>&1 | grep -qi musl || $(CARGO) -vV 2>/dev/null | grep -q musl; then echo musl; else echo gnu; fi)
PKG_NAME := rlshim-$(VERSION)-$(ARCH)-$(LIBC)

RELEASE_BIN := target/release/rlshim
DEBUG_BIN := target/debug/rlshim

all: release

test:
	@echo "Building and running unit tests..."
	$(CARGO) test

release: aur
	@echo "Building Release version..."
	$(CARGO) build --release

debug: aur
	@echo "Building Debug version..."
	$(CARGO) build

aur:
	@mkdir -p $(BUILD_DIR)/aur/rlshim $(BUILD_DIR)/aur/rlshim-bin $(BUILD_DIR)/aur/rlshim-git
	@sed "s/@PROJECT_VERSION@/$(VERSION)/g" packaging/aur/PKGBUILD.source.in > $(BUILD_DIR)/aur/rlshim/PKGBUILD
	@sed "s/@PROJECT_VERSION@/$(VERSION)/g" packaging/aur/PKGBUILD.bin.in > $(BUILD_DIR)/aur/rlshim-bin/PKGBUILD
	@sed "s/@PROJECT_VERSION@/$(VERSION)/g" packaging/aur/PKGBUILD.git.in > $(BUILD_DIR)/aur/rlshim-git/PKGBUILD

clean:
	$(CARGO) clean
	rm -rf $(BUILD_DIR) build-tests

define stage_files
	install -Dm755 "$(1)" "$(2)/bin/rlshim"
	install -Dm644 rlshim.desktop "$(2)/share/applications/rlshim.desktop"
	install -Dm644 data/icon.png "$(2)/share/pixmaps/rlshim.png"
	install -d "$(2)/share/rlshim/data"
	install -m644 data/* "$(2)/share/rlshim/data/"
endef

install:
	@if [ -f "$(RELEASE_BIN)" ]; then \
		BIN="$(RELEASE_BIN)"; \
	elif [ -f "$(DEBUG_BIN)" ]; then \
		BIN="$(DEBUG_BIN)"; \
	else \
		echo "Error: binary not built. Run 'make' first." >&2; exit 1; \
	fi; \
	install -Dm755 "$$BIN" "$(DESTDIR)$(PREFIX)/bin/rlshim"; \
	install -Dm644 rlshim.desktop "$(DESTDIR)$(PREFIX)/share/applications/rlshim.desktop"; \
	install -Dm644 data/icon.png "$(DESTDIR)$(PREFIX)/share/pixmaps/rlshim.png"; \
	install -d "$(DESTDIR)$(PREFIX)/share/rlshim/data"; \
	install -m644 data/* "$(DESTDIR)$(PREFIX)/share/rlshim/data/"

uninstall:
	rm -f "$(DESTDIR)$(PREFIX)/bin/rlshim"
	rm -f "$(DESTDIR)$(PREFIX)/share/applications/rlshim.desktop"
	rm -f "$(DESTDIR)$(PREFIX)/share/pixmaps/rlshim.png"
	rm -rf "$(DESTDIR)$(PREFIX)/share/rlshim"

define build_tar
	@echo "Building tar.gz archive..."
	@rm -rf $(BUILD_DIR)/stage-tar
	@mkdir -p $(BUILD_DIR)/stage-tar/$(PKG_NAME)
	@$(call stage_files,$(1),$(BUILD_DIR)/stage-tar/$(PKG_NAME))
	@tar -C $(BUILD_DIR)/stage-tar -czf $(BUILD_DIR)/$(PKG_NAME).tar.gz $(PKG_NAME)
	@rm -rf $(BUILD_DIR)/stage-tar
endef

define build_deb
	@echo "Building DEB package..."
	@rm -rf $(BUILD_DIR)/stage-deb
	@mkdir -p $(BUILD_DIR)/stage-deb/$(PKG_NAME)/DEBIAN
	@$(call stage_files,$(1),$(BUILD_DIR)/stage-deb/$(PKG_NAME)/usr)
	@printf "Package: rlshim\nVersion: $(VERSION)\nSection: games\nPriority: optional\nArchitecture: $(DEB_ARCH)\nMaintainer: Seraphim Pardee <me@srp.life>\nDepends: libsecret-1-0, default-jre | java11-runtime\nDescription: A lightweight, native Linux launcher for RuneLite.\n" > $(BUILD_DIR)/stage-deb/$(PKG_NAME)/DEBIAN/control
	@dpkg-deb --build --root-owner-group $(BUILD_DIR)/stage-deb/$(PKG_NAME) $(BUILD_DIR)/$(PKG_NAME).deb
	@rm -rf $(BUILD_DIR)/stage-deb
endef

define build_rpm
	@echo "Building RPM package..."
	@rm -rf $(BUILD_DIR)/stage-rpm
	@mkdir -p $(BUILD_DIR)/stage-rpm/BUILD $(BUILD_DIR)/stage-rpm/RPMS $(BUILD_DIR)/stage-rpm/SOURCES $(BUILD_DIR)/stage-rpm/SPECS $(BUILD_DIR)/stage-rpm/SRPMS $(BUILD_DIR)/stage-rpm/BUILDROOT
	@printf "Name: rlshim\nVersion: $(VERSION)\nRelease: 1\nSummary: A lightweight, native Linux launcher for RuneLite.\nLicense: BSD-2-Clause\nURL: https://github.com/RdrSeraphim/rlshim\nRequires: libsecret, java >= 1:11\nAutoReqProv: no\n%%global debug_package %%{nil}\n%%define _build_id_links none\n\n%%description\nA lightweight, native Linux launcher for RuneLite.\n\n%%install\ninstall -Dm755 \"$(CURDIR)/$(1)\" \"%%{buildroot}/usr/bin/rlshim\"\ninstall -Dm644 \"$(CURDIR)/rlshim.desktop\" \"%%{buildroot}/usr/share/applications/rlshim.desktop\"\ninstall -Dm644 \"$(CURDIR)/data/icon.png\" \"%%{buildroot}/usr/share/pixmaps/rlshim.png\"\ninstall -d \"%%{buildroot}/usr/share/rlshim/data\"\ninstall -m644 \"$(CURDIR)\"/data/* \"%%{buildroot}/usr/share/rlshim/data/\"\n\n%%files\n/usr/bin/rlshim\n/usr/share/applications/rlshim.desktop\n/usr/share/pixmaps/rlshim.png\n/usr/share/rlshim\n" > $(BUILD_DIR)/stage-rpm/SPECS/rlshim.spec
	@rpmbuild --define "_topdir $(CURDIR)/$(BUILD_DIR)/stage-rpm" -bb $(BUILD_DIR)/stage-rpm/SPECS/rlshim.spec
	@mv $(BUILD_DIR)/stage-rpm/RPMS/*/*.rpm $(BUILD_DIR)/$(PKG_NAME).rpm
	@rm -rf $(BUILD_DIR)/stage-rpm
endef

deb: release
	$(call build_deb,$(RELEASE_BIN))

rpm: release
	$(call build_rpm,$(RELEASE_BIN))

tar: release
	$(call build_tar,$(RELEASE_BIN))

debug-deb: debug
	$(call build_deb,$(DEBUG_BIN))

debug-rpm: debug
	$(call build_rpm,$(DEBUG_BIN))

debug-tar: debug
	$(call build_tar,$(DEBUG_BIN))

package: deb rpm tar
debug-package: debug-deb debug-rpm debug-tar
