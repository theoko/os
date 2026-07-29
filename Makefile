# Agent-centric OS — freestanding Rust kernel (x86_64 + ARM64), Limine, QEMU
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
# Extra cargo features for the kernel. Since all bridges are removed,
# standalone mode is now the only mode.
KERNEL_FEATURES ?= standalone
ARM64_KERNEL_FEATURES ?= standalone
KERNEL_FEATURE_FLAG := $(if $(KERNEL_FEATURES),--features $(KERNEL_FEATURES),)
ARM64_KERNEL_FEATURE_FLAG := $(if $(ARM64_KERNEL_FEATURES),--features $(ARM64_KERNEL_FEATURES),)
LIMINE_BRANCH := v9.x-binary

QEMU ?= qemu-system-x86_64
QEMUFLAGS ?= -m 512M -serial stdio -display none
QEMU_DEBUG_EXIT := -device isa-debug-exit,iobase=0xf4,iosize=0x04

# ARM64 is UEFI-only: there is no BIOS path and no isa-debug-exit device, so
# the arm64 smoke asserts on serial output alone and stops the VM by timeout.
QEMU_ARM64 ?= qemu-system-aarch64
ARM64_FIRMWARE ?= $(firstword $(wildcard \
	/opt/homebrew/share/qemu/edk2-aarch64-code.fd \
	/usr/local/share/qemu/edk2-aarch64-code.fd \
	/usr/share/qemu/edk2-aarch64-code.fd))
# `ramfb` is what gives Limine a framebuffer to hand the kernel on `virt`.
# The serial is firmware/Limine output only: the guest's own PL011 stays dark
# until the kernel maps device MMIO (Limine's HHDM covers RAM, not MMIO).
ARM64_QEMUFLAGS ?= -M virt -cpu cortex-a72 -m 512M -device ramfb -serial stdio

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

.PHONY: all build kernel arm64-kernel iso arm64-iso iso-arm64 standalone-iso standalone-arm64-iso virtualbox-arm64 run run-arm64 run-best utm utm-run usb usb-list drive drive-selftest linux-vm linux-iso test test-all test-host smoke smoke-arm64 e2e publish-os check-published check-published-install check-published-uninstall clean distclean

all: build

build: iso

kernel:
	$(WITH_RUST) $(CARGO) build -p kernel --target $(KERNEL_TARGET) --profile $(KERNEL_PROFILE) $(KERNEL_FEATURE_FLAG)

# Apple Silicon VirtualBox virtualises ARM guests. This parallel target keeps
# the existing x86 image intact while producing the ARM64 kernel binary.
arm64-kernel:
	$(WITH_RUST) $(CARGO) build -p kernel --target $(ARM64_KERNEL_TARGET) --profile $(ARM64_KERNEL_PROFILE) $(ARM64_KERNEL_FEATURE_FLAG)

# Explicit standalone-named ISO. Default `make iso` already enables standalone;
# these targets keep a distinct filename for people who still keep an old
# non-standalone image next to it.
standalone-iso:
	$(MAKE) KERNEL_FEATURES=standalone IMAGE_NAME=$(IMAGE_NAME)-standalone iso

standalone-arm64-iso:
	$(MAKE) KERNEL_FEATURES=standalone ARM64_IMAGE_NAME=$(ARM64_IMAGE_NAME)-standalone arm64-iso

iso: limine/limine kernel
	rm -rf iso_root
	mkdir -p iso_root/boot/limine iso_root/EFI/BOOT
	cp -f $(KERNEL_ELF) iso_root/boot/kernel
	cp -f limine.conf iso_root/boot/limine/
	@if [ -n "$(RESOLUTION)" ]; then \
		printf '    resolution: %s\n' "$(RESOLUTION)" >> iso_root/boot/limine/limine.conf; \
		echo "boot resolution: $(RESOLUTION)"; \
	fi
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
	@if [ -n "$(RESOLUTION)" ]; then \
		printf '    resolution: %s\n' "$(RESOLUTION)" >> arm64_iso_root/boot/limine/limine.conf; \
		echo "boot resolution: $(RESOLUTION)"; \
	fi
	cp -f limine/limine-uefi-cd.bin arm64_iso_root/boot/limine/
	# El Torito boots from the FAT image above; this copy is what makes the
	# same ISO bootable once written to a USB stick, where firmware looks for
	# a real EFI system partition path instead.
	cp -f limine/BOOTAA64.EFI arm64_iso_root/EFI/BOOT/
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

# Boot the ARM64 ISO under QEMU's `virt` machine. Needs edk2 firmware because
# Limine on aarch64 is UEFI-only; without it the machine sits at a blank
# console with no indication why.
run-arm64: arm64-iso
ifeq ($(ARM64_FIRMWARE),)
	@echo "error: edk2-aarch64-code.fd not found — install qemu (brew install qemu)" >&2; exit 1
else
	$(QEMU_ARM64) $(ARM64_QEMUFLAGS) \
		-drive if=pflash,format=raw,readonly=on,file=$(ARM64_FIRMWARE) \
		-cdrom $(ARM64_IMAGE_NAME).iso || true
endif

# One entrypoint for a person rather than a VM compatibility quiz. VirtualBox
# cannot execute this x86_64 guest on Apple Silicon; UTM can emulate it and is
# the supported desktop route. Other hosts retain the lightweight QEMU path.
run-best:
	@if [ "$(shell uname -s)" = Darwin ] && [ "$(shell uname -m)" = arm64 ]; then \
		echo "Apple Silicon detected: launching the x86_64 OS in UTM."; \
		$(MAKE) utm; \
	else \
		echo "Launching with QEMU."; \
		$(MAKE) run; \
	fi

# Boot the ISO and drive it: setup journey, capability grants, a real query,
# with a screenshot after every step. Catches what unit tests structurally
# cannot - anything only visible from in front of the screen.
drive: iso
	chmod +x scripts/drive-ui.py scripts/e2e/driver.py scripts/e2e/selftest.py
	./scripts/drive-ui.py --out $(DRIVE_OUT) $(DRIVE_ARGS)

# Prove the screen checks can go red. Replays the frames the last `make drive`
# captured, injects each bug class into the pixels, and fails if any assertion
# survives its own defect. A test that cannot fail is worse than no test.
drive-selftest:
	./scripts/e2e/selftest.py --frames $(DRIVE_OUT)

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

# Substrate proof for docs/linux-os-doc-v02.md: aarch64 Linux under Apple's
# hypervisor, to check the frame ceiling is emulation and not our kernel.
linux-vm:
	chmod +x scripts/linux-vm.sh
	./scripts/linux-vm.sh -serial stdio -display none

# Live Debian ISO via the guest build host. Stamps the host git commit into
# /etc/teddyos-software so a fresh install does not offer a false update.
#
#   make linux-iso                 native arch of the guest
#   make linux-iso ARCH=amd64      cross-build on an arm64 guest
linux-iso:
	chmod +x scripts/build-linux-iso.sh
	./scripts/build-linux-iso.sh $(if $(ARCH),--arch $(ARCH),)

test: test-host smoke

# Everything `test` covers, plus proof the same sources boot on aarch64.
test-all: test arm64-kernel smoke-arm64

# Screen-level invariants: boot the real ISO, derive where you are from what is
# drawn, drive it, and assert against the rendered result.
#
# Deliberately NOT part of `make test`. It needs QEMU and macOS's
# text recogniser and several minutes; wiring it into the fast loop would mean
# people stop running the fast loop. Run it before shipping a UI change, and
# after any redesign that moves a control.
#
#   make e2e                          full run, then the proof each assertion
#                                     can fail, evidence in /tmp/os-e2e
#   make e2e E2E_OUT=/tmp/shots       put the evidence somewhere else
#   make e2e E2E_ARGS="--no-sweep"    skip the click-everything wiring sweep
#   make e2e E2E_ARGS="--key-delay 0.02"
#                                     type faster than the guest can keep up,
#                                     which is how the dropped-keystroke
#                                     assertion was shown going red
#
# Exits non-zero if any assertion fails, or if any assertion stays green when
# its evidence is deliberately broken.
DRIVE_OUT ?= /tmp/os-ui
DRIVE_ARGS ?=
E2E_OUT ?= /tmp/os-e2e
E2E_ARGS ?=
e2e: iso
	chmod +x scripts/e2e/invariants.py
	./scripts/e2e/invariants.py --out $(E2E_OUT) $(E2E_ARGS)

# Two passes on purpose. The bare pass keeps the offline/COM2 stub paths type-
# checked while they still exist; the second is the configuration every shipped
# ISO is built in. Without it, `standalone()` branches only compile during
# shipping — which is how a nav-dot bug that only shows under standalone once
# reached first boot.
test-host:
	$(WITH_RUST) $(CARGO) test -p kernel --target $$($(RUSTC) -vV | awk '/^host:/{print $$2}') --lib
	$(WITH_RUST) $(CARGO) test -p kernel --target $$($(RUSTC) -vV | awk '/^host:/{print $$2}') --lib --features standalone
	$(WITH_RUST) $(CARGO) test -p os-core

smoke: iso
	chmod +x scripts/smoke-qemu.sh
	./scripts/smoke-qemu.sh

# Boots the same kernel sources on aarch64. Kept out of `test` because it needs
# edk2 firmware that not every host has; `make test-all` runs both.
smoke-arm64: arm64-iso
	chmod +x scripts/smoke-arm64.sh
	./scripts/smoke-arm64.sh

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

# Publish both ISOs to the public download page.
#
# Build release images from a clean tree, BOOT both, checksum them, upload
# beside the live files and move into place, then verify on the server and
# again over HTTPS. The boot test is the reason this is a target and not a
# runbook: shipping an image that does not boot is the one failure here that
# stays silent until a stranger hits it.
#
#   make publish-os                     the whole thing
#   make publish-os DRY_RUN=1           build, boot, checksum; upload nothing
#   make publish-os ALLOW_DIRTY=1       publish from an uncommitted tree
#   make publish-os PUBLISH_HOST=... PUBLISH_DIR=... PUBLISH_URL=...
publish-os:
	chmod +x scripts/publish-os.sh
	./scripts/publish-os.sh

# Bring a published image back down, verified. The other half of publish-os.
#
# Downloads the image for this architecture from the same URL a visitor uses,
# checks it against the published checksums, and stages it outside the repo.
# It applies nothing: these are boot media, and what a machine boots from is
# not something a status check gets to change.
#
# The URL carries ?v=<commit>, because a CDN fronts the origin and the bare URL
# serves the previous release for hours — the same reason os.html links the
# versioned form. Without it a good release downloads as a checksum mismatch,
# which reads to anyone verifying it as tampering.
#
#   make update-os                 for this machine
#   make update-os ARCH=x86_64     for the other one
#   make update-check              report only, download nothing
update-os:
	chmod +x scripts/update-os.sh
	./scripts/update-os.sh $(if $(ARCH),--arch $(ARCH),)

update-check:
	chmod +x scripts/update-os.sh
	./scripts/update-os.sh --check

# Watch the published download from outside, on a timer.
#
# Everything it checks has already broken once: the CDN served an older image
# than the checksums advertised, and the homepage link lives in a file other
# work deploys by rsync. Both failures are silent — the page keeps returning
# 200 while being wrong or unreachable.
#
#   make check-published          run it once, now
#   make check-published-install  every 6h, notifies on failure and recovery
#   make check-published-uninstall
check-published:
	chmod +x scripts/check-published.sh
	./scripts/check-published.sh

# Installs a COPY of the scripts outside ~/Desktop, and points the timer there.
#
# launchd cannot execute anything under ~/Desktop, ~/Documents or ~/Downloads
# without Full Disk Access: the job loads, dies with "Operation not permitted"
# (exit 126), and looks installed while doing nothing — into a log it also
# cannot write, so there is no trace either. `launchctl list` shows exactly
# that against com.os.refresh, which has never run for this reason.
#
# The copy means editing a script does not change what is scheduled: re-run
# this target to publish the edit.
MONITOR_DIR := $(HOME)/Library/Application Support/os/monitor
MONITOR_LOG := $(HOME)/Library/Logs/os-check-published.log

check-published-install:
	mkdir -p "$(MONITOR_DIR)" $(HOME)/Library/LaunchAgents $(HOME)/Library/Logs
	cp scripts/check-published.sh scripts/check-published-notify.sh "$(MONITOR_DIR)/"
	chmod +x "$(MONITOR_DIR)/check-published.sh" "$(MONITOR_DIR)/check-published-notify.sh"
	sed -e 's#__INSTALL_DIR__#$(MONITOR_DIR)#g' -e 's#__LOG__#$(MONITOR_LOG)#g' \
		deploy/com.os.check-published.plist > $(HOME)/Library/LaunchAgents/com.os.check-published.plist
	launchctl unload $(HOME)/Library/LaunchAgents/com.os.check-published.plist 2>/dev/null || true
	launchctl load $(HOME)/Library/LaunchAgents/com.os.check-published.plist
	@echo ">>> every 6h — log: $(MONITOR_LOG); notifies only on change"
	@echo ">>> re-run this target after editing either script (it installs a copy)"

check-published-uninstall:
	launchctl unload $(HOME)/Library/LaunchAgents/com.os.check-published.plist 2>/dev/null || true
	rm -f $(HOME)/Library/LaunchAgents/com.os.check-published.plist
	rm -rf "$(MONITOR_DIR)"
	@echo ">>> monitor removed"

clean:
	$(WITH_RUST) $(CARGO) clean
	rm -rf iso_root $(IMAGE_NAME).iso serial.out .smoke-*
	rm -rf arm64_iso_root $(ARM64_IMAGE_NAME).iso

distclean: clean
	rm -rf limine edk2-ovmf
