# teddyOS — Linux daily driver only.
#
# macOS /usr/bin/make (BSD) does not apply `export PATH := …` to direct execvp
# recipe lines for some tools; keep recipes as plain shell scripts.
MAKEFLAGS += --no-builtin-rules
.SUFFIXES:

.PHONY: all help desktop desktop-force linux-init linux-provision linux-iso linux-vm \
	test test-host publish-os update-os update-check \
	check-published check-published-install check-published-uninstall clean

# Default: open the Linux guest in UTM.
all: desktop

help:
	@echo "teddyOS — Linux daily driver"
	@echo ""
	@echo "  make desktop           open Linux guest in UTM (VM: teddyos)"
	@echo "  make desktop-force     recreate UTM VM + re-copy disk (repair)"
	@echo "  make linux-init        first-time Debian disk + cloud-init"
	@echo "  make linux-provision   GNOME + Chromium (headless QEMU + ssh :2222)"
	@echo "  make linux-iso         build live ISO via guest (dist/)"
	@echo "  make test              Linux contract unit tests on the host"
	@echo "  make publish-os        publish live images + update payload"

desktop:
	chmod +x scripts/teddyos-desktop.sh scripts/teddyos-utm.sh scripts/teddyos-vm.sh
	./scripts/teddyos-desktop.sh --desktop

# Recreate the UTM VM + re-copy disk (fixes ghosts / frozen black display).
desktop-force:
	chmod +x scripts/teddyos-desktop.sh scripts/teddyos-utm.sh scripts/teddyos-vm.sh
	./scripts/teddyos-desktop.sh --desktop --force

linux-init:
	chmod +x scripts/teddyos-desktop.sh scripts/teddyos-vm.sh
	./scripts/teddyos-desktop.sh --init

linux-provision:
	chmod +x scripts/teddyos-desktop.sh scripts/teddyos-provision.sh
	./scripts/teddyos-desktop.sh --provision

# Live Debian ISO via the guest build host.
#   make linux-iso
#   make linux-iso ARCH=amd64
linux-iso:
	chmod +x scripts/build-linux-iso.sh
	./scripts/build-linux-iso.sh $(if $(ARCH),--arch $(ARCH),)

# Optional HVF smoke: plain Alpine aarch64 (not teddyOS apps).
linux-vm:
	chmod +x scripts/linux-vm.sh
	./scripts/linux-vm.sh -serial stdio -display none

# Host-side Linux contracts (no freestanding kernel / QEMU ISO).
test test-host:
	chmod +x scripts/linux-contract-unit.sh
	./scripts/linux-contract-unit.sh

publish-os:
	chmod +x scripts/publish-os.sh
	./scripts/publish-os.sh $(if $(DRY_RUN),DRY_RUN=$(DRY_RUN),) $(if $(ALLOW_DIRTY),ALLOW_DIRTY=$(ALLOW_DIRTY),)

update-os:
	chmod +x scripts/update-os.sh
	./scripts/update-os.sh

update-check:
	chmod +x scripts/check-published.sh
	./scripts/check-published.sh

check-published:
	chmod +x scripts/check-published.sh
	./scripts/check-published.sh

check-published-install:
	chmod +x scripts/check-published.sh
	./scripts/check-published.sh --install

check-published-uninstall:
	chmod +x scripts/check-published.sh
	./scripts/check-published.sh --uninstall

clean:
	rm -f .teddyos-commit serial.out
	rm -rf dist/*.iso dist/*.tar.gz 2>/dev/null || true
	@echo "Linux guest disk (.teddyos-vm/) is left in place — remove it by hand if you want a full reset."
