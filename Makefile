# Agent-centric OS — x86_64 Limine + QEMU + host MCP bridge
#
# macOS /usr/bin/make (BSD) does not apply `export PATH := …` to direct execvp
# recipe lines, so bare `cargo` fails when rustup is not on the ambient PATH.
MAKEFLAGS += --no-builtin-rules
.SUFFIXES:

IMAGE_NAME := os
KERNEL_TARGET := x86_64-unknown-none
KERNEL_PROFILE ?= dev
KERNEL_PROFILE_DIR := $(if $(filter dev,$(KERNEL_PROFILE)),debug,$(KERNEL_PROFILE))
KERNEL_ELF := target/$(KERNEL_TARGET)/$(KERNEL_PROFILE_DIR)/kernel
ARM64_KERNEL_TARGET := aarch64-unknown-none
# VirtualBox is the interactive ARM product path, so ship its optimized build
# by default while preserving the x86 developer profile and override knobs.
ARM64_KERNEL_PROFILE ?= release
ARM64_KERNEL_PROFILE_DIR := $(if $(filter dev,$(ARM64_KERNEL_PROFILE)),debug,$(ARM64_KERNEL_PROFILE))
ARM64_KERNEL_ELF := target/$(ARM64_KERNEL_TARGET)/$(ARM64_KERNEL_PROFILE_DIR)/kernel
ARM64_IMAGE_NAME := os-arm64
LIMINE_BRANCH := v9.x-binary
BRIDGE_ADDR ?= 127.0.0.1:7420
# `auto` selects Gmail when the host already has a signed-in gog account;
# otherwise the OS stays honest that email still needs connecting.
EMAIL_BACKEND ?= auto

QEMU ?= qemu-system-x86_64
QEMUFLAGS ?= -m 512M -serial stdio -display none
QEMU_DEBUG_EXIT := -device isa-debug-exit,iobase=0xf4,iosize=0x04

CARGO ?= $(firstword $(wildcard \
	/opt/homebrew/opt/rustup/bin/cargo \
	$(HOME)/.cargo/bin/cargo \
	/usr/local/opt/rustup/bin/cargo) \
	$(shell command -v cargo 2>/dev/null))
RUSTC ?= $(firstword $(wildcard \
	/opt/homebrew/opt/rustup/bin/rustc \
	$(HOME)/.cargo/bin/rustc \
	/usr/local/opt/rustup/bin/rustc) \
	$(shell command -v rustc 2>/dev/null))

ifeq ($(CARGO),)
$(error cargo not found — install rustup: brew install rustup && rustup default stable && rustup target add x86_64-unknown-none)
endif

RUSTUP_BIN := $(patsubst %/,%,$(dir $(CARGO)))
WITH_RUST := PATH="$(RUSTUP_BIN):$$PATH"

.PHONY: all build kernel arm64-kernel iso arm64-iso iso-arm64 virtualbox-arm64 bridge bridge-run run run-bridged run-best utm utm-run utm-bridged usb usb-list drive linux-vm refresh refresh-install refresh-uninstall test test-host smoke smoke-bridge clean distclean

all: build

build: iso

kernel:
	$(WITH_RUST) $(CARGO) build -p kernel --target $(KERNEL_TARGET) --profile $(KERNEL_PROFILE)

# Apple Silicon VirtualBox virtualises ARM guests. This parallel target keeps
# the existing x86 image intact while producing the ARM64 kernel binary.
arm64-kernel:
	$(WITH_RUST) $(CARGO) build -p kernel --target $(ARM64_KERNEL_TARGET) --profile $(ARM64_KERNEL_PROFILE)

bridge:
	$(WITH_RUST) $(CARGO) build -p os-mcp-bridge

bridge-run: bridge
	OS_MCP_BRIDGE_ADDR=$(BRIDGE_ADDR) OS_MCP_EMAIL_BACKEND=$(EMAIL_BACKEND) \
		$(WITH_RUST) $(CARGO) run -p os-mcp-bridge

iso: limine/limine kernel
	rm -rf iso_root
	mkdir -p iso_root/boot/limine iso_root/EFI/BOOT
	cp -f $(KERNEL_ELF) iso_root/boot/kernel
	cp -f limine.conf iso_root/boot/limine/
	cp -f limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin \
		iso_root/boot/limine/
	cp -f limine/BOOTX64.EFI iso_root/EFI/BOOT/
	xorriso -as mkisofs \
		-R -r -J \
		-b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		iso_root -o $(IMAGE_NAME).iso
	./limine/limine bios-install $(IMAGE_NAME).iso
	rm -rf iso_root

# ARM64 UEFI-only ISO. Limine's FAT UEFI image contains BOOTAA64.EFI; ARM
# firmware recognises this El Torito form, whereas a bare PE file is not a
# mountable EFI system partition on all virtual CD-ROM implementations.
arm64-iso: limine/limine arm64-kernel
	rm -rf arm64_iso_root
	mkdir -p arm64_iso_root/boot/limine arm64_iso_root/EFI/BOOT
	cp -f $(ARM64_KERNEL_ELF) arm64_iso_root/boot/kernel
	cp -f limine.conf arm64_iso_root/boot/limine/
	cp -f limine/limine-uefi-cd.bin arm64_iso_root/boot/limine/
	xorriso -as mkisofs -R -r -J \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		-o $(ARM64_IMAGE_NAME).iso arm64_iso_root
	rm -rf arm64_iso_root

# Familiar word order for the artifact name: `make iso-arm64`.
iso-arm64: arm64-iso

# Native ARM64 VirtualBox path for Apple Silicon. The helper creates or
# refreshes a correctly configured OHCI + QemuRamFB VM and reattaches the ISO
# so VirtualBox never boots stale medium contents.
virtualbox-arm64: iso-arm64
	chmod +x scripts/make-virtualbox-arm64.sh
	./scripts/make-virtualbox-arm64.sh

run: iso
	$(QEMU) -M q35 -cdrom $(IMAGE_NAME).iso -boot d $(QEMUFLAGS) $(QEMU_DEBUG_EXIT) || true

# COM1 = stdio, COM2 = TCP client → host MCP bridge (start bridge first, or use smoke-bridge).
run-bridged: iso bridge
	@echo "Start bridge in another terminal: make bridge-run EMAIL_BACKEND=gog"
	@echo "Or with mock: make bridge-run"
	$(QEMU) -M q35 -cdrom $(IMAGE_NAME).iso -boot d \
		-m 512M -display none \
		-serial stdio \
		-serial tcp:$(BRIDGE_ADDR) \
		$(QEMU_DEBUG_EXIT) || true

# One entrypoint for a person rather than a VM compatibility quiz. VirtualBox
# cannot execute this x86_64 guest on Apple Silicon; UTM can emulate it and is
# the supported desktop route. Other hosts retain the lightweight QEMU path.
run-best:
	@if [ "$(shell uname -s)" = Darwin ] && [ "$(shell uname -m)" = arm64 ]; then \
		echo "Apple Silicon detected: launching the x86_64 OS in UTM."; \
		$(MAKE) utm-bridged; \
	else \
		echo "Launching with QEMU."; \
		$(MAKE) run-bridged; \
	fi

# Boot the ISO and drive it: setup journey, capability grants, a real query,
# with a screenshot after every step. Catches what unit tests structurally
# cannot - anything only visible from in front of the screen.
drive: iso bridge
	chmod +x scripts/drive-ui.py scripts/ensure-bridge.sh
	./scripts/drive-ui.py

# Write the ISO to a USB stick for real x86-64 hardware. Destructive, so it
# refuses internal disks and makes you retype the device before writing.
usb: iso
	chmod +x scripts/make-usb.sh
	./scripts/make-usb.sh

usb-list:
	chmod +x scripts/make-usb.sh
	./scripts/make-usb.sh --list

utm: iso
	chmod +x scripts/make-utm.sh
	./scripts/make-utm.sh

utm-run: iso
	chmod +x scripts/make-utm.sh
	UTM_START=1 ./scripts/make-utm.sh

# Host bridge on TCP :7420; UTM COM2 = Serial TcpClient to that address.
utm-bridged: iso bridge
	chmod +x scripts/ensure-bridge.sh scripts/make-utm.sh
	# The helper waits for an existing bridge rather than replacing one while it
	# is still binding. It automatically uses Gmail when an account is connected
	# and otherwise tells the user email needs connecting (no fake inbox data).
	OS_MCP_BRIDGE_ADDR=$(BRIDGE_ADDR) EMAIL_BACKEND=$(EMAIL_BACKEND) ./scripts/ensure-bridge.sh
	UTM_BRIDGE=1 UTM_START=1 OS_MCP_BRIDGE_ADDR=$(BRIDGE_ADDR) ./scripts/make-utm.sh

# Substrate proof for docs/linux-os-doc-v02.md: aarch64 Linux under Apple's
# hypervisor, to check the frame ceiling is emulation and not our kernel.
linux-vm:
	chmod +x scripts/linux-vm.sh
	./scripts/linux-vm.sh -serial stdio -display none

# Daily search refresh. Re-indexes workspace files and re-syncs the portal
# corpus, but only if that corpus was already synced once — a host-side timer
# must not be a way to start talking to the network on the user's behalf.
refresh:
	./scripts/refresh-index.sh

refresh-install:
	mkdir -p $(HOME)/Library/LaunchAgents
	cp deploy/com.os.refresh.plist $(HOME)/Library/LaunchAgents/
	launchctl unload $(HOME)/Library/LaunchAgents/com.os.refresh.plist 2>/dev/null || true
	launchctl load $(HOME)/Library/LaunchAgents/com.os.refresh.plist
	@echo ">>> daily at 08:00 — log: .refresh.log"

refresh-uninstall:
	launchctl unload $(HOME)/Library/LaunchAgents/com.os.refresh.plist 2>/dev/null || true
	rm -f $(HOME)/Library/LaunchAgents/com.os.refresh.plist
	@echo ">>> removed"

test: test-host smoke smoke-bridge

test-host:
	$(WITH_RUST) $(CARGO) test -p kernel --target $$($(RUSTC) -vV | awk '/^host:/{print $$2}') --lib
	$(WITH_RUST) $(CARGO) test -p os-mcp-bridge

smoke: iso
	chmod +x scripts/smoke-qemu.sh
	./scripts/smoke-qemu.sh

smoke-bridge: iso bridge
	chmod +x scripts/smoke-bridge.sh
	./scripts/smoke-bridge.sh

# An existing checkout is never auto-refreshed, so at least surface a
# LIMINE_BRANCH mismatch instead of silently building with the old bootloader.
LIMINE_HAVE := $(shell git -C limine rev-parse --abbrev-ref HEAD 2>/dev/null)
ifneq ($(LIMINE_HAVE),)
ifneq ($(LIMINE_HAVE),$(LIMINE_BRANCH))
$(warning limine checkout is on '$(LIMINE_HAVE)' but LIMINE_BRANCH is '$(LIMINE_BRANCH)' — run 'rm -rf limine' to re-clone)
endif
endif

limine/limine:
	rm -rf limine
	git clone https://github.com/limine-bootloader/limine.git --branch=$(LIMINE_BRANCH) --depth=1 limine
	$(MAKE) -C limine

clean:
	$(WITH_RUST) $(CARGO) clean
	rm -rf iso_root $(IMAGE_NAME).iso serial.out .smoke-*

distclean: clean
	rm -rf limine edk2-ovmf
