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
LIMINE_BRANCH := v9.x-binary
BRIDGE_ADDR ?= 127.0.0.1:7420
EMAIL_BACKEND ?= mock

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

.PHONY: all build kernel iso bridge bridge-run run run-bridged utm utm-run utm-bridged test test-host smoke smoke-bridge clean distclean

all: build

build: iso

kernel:
	$(WITH_RUST) $(CARGO) build -p kernel --target $(KERNEL_TARGET) --profile $(KERNEL_PROFILE)

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

run: iso
	$(QEMU) -M q35 -cdrom $(IMAGE_NAME).iso -boot d $(QEMUFLAGS) $(QEMU_DEBUG_EXIT) || true

# COM1 = stdio, COM2 = TCP server; host bridge dials (same topology as UTM).
run-bridged: iso bridge
	OS_MCP_BRIDGE_CONNECT=tcp:$(BRIDGE_ADDR) ./scripts/ensure-bridge.sh
	$(QEMU) -M q35 -cdrom $(IMAGE_NAME).iso -boot d \
		-m 512M -display none \
		-serial stdio \
		-serial tcp:$(BRIDGE_ADDR),server \
		$(QEMU_DEBUG_EXIT) || true

utm: iso
	./scripts/make-utm.sh

utm-run: iso
	UTM_START=1 ./scripts/make-utm.sh

# COM2 TcpServer + dialing bridge (retries). One wiring model for QEMU and UTM.
utm-bridged: iso bridge
	OS_MCP_BRIDGE_CONNECT=tcp:$(BRIDGE_ADDR) ./scripts/ensure-bridge.sh
	UTM_START=1 OS_MCP_BRIDGE_ADDR=$(BRIDGE_ADDR) ./scripts/make-utm.sh

test: test-host smoke smoke-bridge

test-host:
	$(WITH_RUST) $(CARGO) test -p kernel --target $$($(RUSTC) -vV | awk '/^host:/{print $$2}') --lib
	$(WITH_RUST) $(CARGO) test -p os-mcp-bridge

smoke: iso
	./scripts/smoke-qemu.sh

smoke-bridge: iso bridge
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
	rm -rf limine
