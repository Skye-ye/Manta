# Building variables
FEATURES :=

ifneq ($(STRACE), )
	FEATURES += strace
endif
ifneq ($(SMP), )
	FEATURES += smp
endif
ifneq ($(PREEMPT), )
	FEATURES += preempt
endif
ifneq ($(DEBUG), )
	FEATURES += debug
endif
ifneq ($(VF2), )
	FEATURES += vf2
endif
ifneq ($(FINAL2), )
	FEATURES += final2
endif

CARGO_BUILD_ARGS :=
ifeq ($(MODE), release)
	 CARGO_BUILD_ARGS += --release
endif
ifneq ($(FEATURES), )
	CARGO_BUILD_ARGS += --features "$(FEATURES)"
endif

# Add the arch crate feature based on TARGET_FEATURE
ifneq ($(TARGET_FEATURE), )
	ARCH_FEATURE := --features="$(TARGET_FEATURE)" -p arch
endif

# Phony targets
PHONY :=

PHONY := all
all: build

PHONY += build
build:
	# First build arch with correct feature
	cargo build $(CARGO_BUILD_ARGS) $(ARCH_FEATURE)
	# Then build kernel
	cargo build $(CARGO_BUILD_ARGS)

.PHONY: $(PHONY)
