# Building variables
DOCKER_NAME = manta
BOARD := qemu
ARCH ?= riscv64

NET ?= n # Enable VirtioNet device, use local Loopback device if disabled
CPUS := 2
MEM := 128M
DISK_2 ?= n

# Set target architecture based on ARCH
ifeq ($(ARCH),riscv64)
export TARGET = riscv64gc-unknown-none-elf
OBJDUMP = rust-objdump --arch-name=riscv64
OBJCOPY = rust-objcopy --binary-architecture=riscv64
ARCH_GDB ?= riscv64-unknown-elf-gdb
QEMU = qemu-system-riscv64
BLK_DEVICE = virtio-blk-device
NET_DEVICE = virtio-net-device
MACHINE = virt
BIOS = default
else ifeq ($(ARCH),loongarch64)
export TARGET = loongarch64-unknown-none
OBJDUMP = rust-objdump --arch-name=loongarch64
OBJCOPY = rust-objcopy --binary-architecture=loongarch64
ARCH_GDB ?= loongarch64-unknown-elf-gdb
QEMU = qemu-system-loongarch64
BLK_DEVICE = virtio-blk-pci
NET_DEVICE = virtio-net-pci
MACHINE = virt
BIOS = none
else
$(error Unsupported architecture: $(ARCH). Supported: riscv64, loongarch64)
endif

export MODE = release
export LOG = error

export Phoenix_IP=$(IP)
export Phoenix_GW=$(GW)

# Tools
PAGER ?= less

# Target files
TARGET_DIR := ./target/$(TARGET)/$(MODE)
VENDOR_DIR := ./third-party/vendor

KERNEL_ELF := $(TARGET_DIR)/kernel
KERNEL_BIN := $(KERNEL_ELF).bin
KERNEL_ASM := $(KERNEL_ELF).asm

USER_APPS_DIR := ./user/src/bin
USER_APPS := $(wildcard $(USER_APPS_DIR)/*.rs)
USER_ELFS := $(patsubst $(USER_APPS_DIR)/%.rs, $(TARGET_DIR)/%, $(USER_APPS))
USER_BINS := $(patsubst $(USER_APPS_DIR)/%.rs, $(TARGET_DIR)/%.bin, $(USER_APPS))

FS_IMG_DIR := .
FS_IMG := $(FS_IMG_DIR)/sdcard.img
TEST := 24/final
# FS := fat32
FS := ext4
SDCARD := n
TEST_DIR := ./testcase/$(TEST)
# TEST_DIR := ./testcase/24/preliminary/

# Crate features
export STRACE := 
export SMP :=
export PREEMPT :=
export DEBUG :=
export FINAL2 :=
export TARGET_FEATURE := $(ARCH)

# Args
DISASM_ARGS = -d

QEMU_ARGS :=
QEMU_ARGS += -m $(MEM)
QEMU_ARGS += -nographic
QEMU_ARGS += -smp $(CPUS)
QEMU_ARGS += -kernel $(KERNEL_BIN)
QEMU_ARGS += -drive file=$(FS_IMG),if=none,format=raw,id=x0
QEMU_ARGS += -device $(BLK_DEVICE),drive=x0,bus=virtio-mmio-bus.0
QEMU_ARGS += -rtc base=utc
QEMU_ARGS += -no-reboot

ifeq ($(ARCH),riscv64)
QEMU_ARGS += -machine $(MACHINE)
QEMU_ARGS += -bios $(BIOS)
endif

# Add second disk
ifeq ($(DISK_2),y)
DISK_IMG := $(FS_IMG_DIR)/disk.img
ifeq ($(ARCH),loongarch64)
DISK_IMG := $(FS_IMG_DIR)/disk-la.img
endif

QEMU_ARGS += -drive file=$(DISK_IMG),if=none,format=raw,id=x1
QEMU_ARGS += -device $(BLK_DEVICE),drive=x1,bus=virtio-mmio-bus.1
endif

# Net
IP ?= 10.0.2.15
GW ?= 10.0.2.2

ifeq ($(NET),y)
$(info "enabled qemu net device")
ifeq ($(ARCH),riscv64)
# Configuration for RISC-V
QEMU_ARGS += -device $(NET_DEVICE),netdev=net
QEMU_ARGS += -netdev user,id=net
else ifeq ($(ARCH),loongarch64)
# Configuration for LoongArch
QEMU_ARGS += -device $(NET_DEVICE),netdev=net0
QEMU_ARGS += -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555
endif
QEMU_ARGS += -d guest_errors
QEMU_ARGS += -d unimp
endif

DOCKER_RUN_ARGS := run
DOCKER_RUN_ARGS += --rm
DOCKER_RUN_ARGS += -it
DOCKER_RUN_ARGS += --privileged
DOCKER_RUN_ARGS += --network="host"
DOCKER_RUN_ARGS += -v $(PWD):/mnt
DOCKER_RUN_ARGS += -v /dev:/dev
DOCKER_RUN_ARGS += -w /mnt
DOCKER_RUN_ARGS += $(DOCKER_NAME)
DOCKER_RUN_ARGS += bash


# File targets
$(KERNEL_ASM): $(KERNEL_ELF)
	@$(OBJDUMP) $(DISASM_ARGS) $(KERNEL_ELF) > $(KERNEL_ASM)
	@echo "Updated: $(KERNEL_ASM)"


# Phony targets
PHONY := all
all: build run MODE=release

PHONY += build_docker
build_docker:
	docker build --network="host" -t ${DOCKER_NAME} .

PHONY += docker
docker:
	docker $(DOCKER_RUN_ARGS)

PHONY += env
env:
	@(cargo install --list | grep "cargo-binutils" > /dev/null 2>&1) || cargo install cargo-binutils
	@cargo vendor $(VENDOR_DIR)

PHONY += fmt
fmt:
	@cargo fmt

PHONY += build
build: fmt user fs-img kernel

PHONY += kernel
kernel:
	@echo "building kernel for $(ARCH)..."
	@echo Platform: $(BOARD)
	@cd kernel && make build
	@$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $(KERNEL_BIN)
	@echo "building kernel finished"

PHONY += user
user:
	@echo "building user for $(ARCH)..."
	@cd user && make build
	@$(foreach elf, $(USER_ELFS), $(OBJCOPY) $(elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf));)
	@cp ./testcase/22/busybox $(TARGET_DIR)/busybox
	@echo "building user finished"

PHONY += fs-img
fs-img:
	@echo "building fs-img..."
ifeq ($(SDCARD), n)
	@rm -f $(FS_IMG)
endif
	@mkdir -p $(FS_IMG_DIR)
	@mkdir -p mnt
ifeq ($(FS), fat32)
ifeq ($(SDCARD), n)
	dd if=/dev/zero of=$(FS_IMG) count=1363148 bs=1K
endif
	@mkfs.vfat -F 32 -s 8 $(FS_IMG)
	@echo "making fatfs image by using $(TEST_DIR)"
	@mount -t vfat -o user,umask=000,utf8=1 --source $(FS_IMG) --target mnt
else
ifeq ($(SDCARD), n)
	dd if=/dev/zero of=$(FS_IMG) count=2048 bs=1M
endif
	# @mkfs.ext4 $(FS_IMG)
	@mkfs.ext4 -F -O ^metadata_csum_seed $(FS_IMG)
	@echo "making ext4 image by using $(TEST_DIR)"
	@mount $(FS_IMG) mnt
endif
	@cp -r $(TEST_DIR)/* mnt
	@cp -r $(USER_ELFS) mnt
	@umount mnt
	@rm -rf mnt
	@chmod 777 $(FS_IMG)
	@echo "building fs-img finished"

PHONY += qemu
qemu:
	@echo "start to run kernel in qemu for $(ARCH)..."
	$(QEMU) $(QEMU_ARGS)

PHONY += dumpdtb
dumpdtb:
	$(QEMU) $(QEMU_ARGS) -machine dumpdtb=$(ARCH)-virt.dtb
	dtc -I dtb -O dts -o $(ARCH)-virt.dts $(ARCH)-virt.dtb

PHONY += run
run: qemu

PHONY += brun
brun: fmt clean-cargo user kernel run

PHONY += clean
clean:
	@cargo clean
	@rm -rf $(FS_IMG)
ifeq ($(DISK_2),y)
	@rm -rf $(DISK_IMG)
endif

PHONY += clean-cargo
clean-cargo:
	@cargo clean

PHONY += disasm
disasm: $(KERNEL_ASM)
	@$(PAGER) $(KERNEL_ASM)

PHONY += trace
trace:
	addr2line -fipe $(KERNEL_ELF) | rustfilt

PHONY += drun
drun: fmt clean-cargo user kernel
	$(QEMU) $(QEMU_ARGS) -s -S

PHONY += debug
debug:
	$(QEMU) $(QEMU_ARGS) -s -S

PHONY += gdb
gdb:
ifeq ($(ARCH),riscv64)
	$(ARCH_GDB) -ex 'file $(KERNEL_ELF)' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'
else ifeq ($(ARCH),loongarch64)
	$(ARCH_GDB) -ex 'file $(KERNEL_ELF)' -ex 'set arch loongarch:loongarch64' -ex 'target remote localhost:1234'
endif

PHONY += zImage
zImage: kernel
	gzip -f $(KERNEL_BIN)
	mkimage -A riscv -O linux -C gzip -T kernel -a 0x80200000 -e 0x80200000 -n Manta -d $(KERNEL_BIN).gz zImage
	sudo cp zImage /srv/tftp/

.PHONY: $(PHONY)

