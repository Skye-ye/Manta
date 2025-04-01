# syntax=docker/dockerfile:1
FROM ubuntu:22.04

ARG QEMU_VERSION=9.2.1
ARG HOME=/root

# 0. Set up mirrors and install wget
RUN sed -i s@/archive.ubuntu.com/@/mirrors.tuna.tsinghua.edu.cn/@g /etc/apt/sources.list
RUN sed -i s@/security.ubuntu.com/@/mirrors.tuna.tsinghua.edu.cn/@g /etc/apt/sources.list
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    wget

# 1. Install dependencies and development tools
# - https://gitlab.educg.net/wangmingjian/os-contest-2024-image

# 1.1. Install ca-certificates
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates

RUN update-ca-certificates

# 1.2. Add LLVM 19 repository
RUN echo deb http://apt.llvm.org/jammy/ llvm-toolchain-jammy-19 main >> /etc/apt/sources.list
RUN wget -qO- https://apt.llvm.org/llvm-snapshot.gpg.key | tee /etc/apt/trusted.gpg.d/apt.llvm.org.asc

# 1.3. Install dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    xz-utils git python3 python3-pip  python3-venv build-essential \
    ninja-build pkg-config  libglib2.0-dev  libpixman-1-dev libslirp-dev \
    make sshpass openssh-client libc-dev u-boot-tools bzip2 \
    gdb-multiarch gcc-riscv64-linux-gnu binutils-riscv64-linux-gnu libpixman-1-0 \
    libguestfs-tools qemu-utils linux-image-generic libncurses5-dev \
    autotools-dev automake texinfo tini musl musl-tools musl-dev cmake libclang-19-dev \
    fusefat libvirglrenderer-dev libsdl2-dev libgtk-3-dev device-tree-compiler

# 1.4. Install python dependencies
RUN python3 -m pip config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple
RUN python3 -m pip install tomli pytz Cython jwt jinja2 requests

# 1.5. Install musl
WORKDIR ${HOME}
RUN wget --progress=dot:giga \
    https://gitlab.educg.net/wangmingjian/os-contest-2024-image/-/raw/master/riscv64-linux-musl-cross.tgz \
    https://gitlab.educg.net/wangmingjian/os-contest-2024-image/-/raw/master/loongarch64-linux-musl-cross.tgz && \
    tar xvf riscv64-linux-musl-cross.tgz && \
    tar xvf loongarch64-linux-musl-cross.tgz
RUN rm -rf riscv64-linux-musl-cross.tgz loongarch64-linux-musl-cross.tgz
ENV PATH=${HOME}/riscv64-linux-musl-cross/bin:$PATH
ENV PATH=${HOME}/loongarch64-linux-musl-cross/bin:$PATH

# 1.6. Install gcc-13.2.0-loongarch64-linux-gnu
WORKDIR ${HOME}
RUN wget --progress=dot:giga \
    https://gitlab.educg.net/wangmingjian/os-contest-2024-image/-/raw/master/gcc-13.2.0-loongarch64-linux-gnu.tgz && \
    tar xvf gcc-13.2.0-loongarch64-linux-gnu.tgz
RUN rm -rf gcc-13.2.0-loongarch64-linux-gnu.tgz
ENV PATH=${HOME}/gcc-13.2.0-loongarch64-linux-gnu/bin:$PATH

# 1.7. Install toolchain-loongarch64-linux-gnu-gcc8-host-x86_64
WORKDIR ${HOME}
RUN wget --progress=dot:giga \
    https://gitlab.educg.net/wangmingjian/os-contest-2024-image/-/raw/master/toolchain-loongarch64-linux-gnu-gcc8-host-x86_64-2022-07-18.tar.xz && \
    tar xvf toolchain-loongarch64-linux-gnu-gcc8-host-x86_64-2022-07-18.tar.xz
RUN rm -rf toolchain-loongarch64-linux-gnu-gcc8-host-x86_64-2022-07-18.tar.xz
RUN mv toolchain-loongarch64-linux-gnu-gcc8-host-x86_64-2022-07-18 toolchain-loongarch64-linux-gnu-gcc8-host-x86_64
ENV PATH=${HOME}/toolchain-loongarch64-linux-gnu-gcc8-host-x86_64/bin:$PATH

# 1.8. Install riscv64-musl-bleeding-edge
WORKDIR ${HOME}
RUN wget --progress=dot:giga \
    https://gitlab.educg.net/wangmingjian/os-contest-2024-image/-/raw/master/riscv64--musl--bleeding-edge-2020.08-1.tar.bz2 && \
    tar jxvf riscv64--musl--bleeding-edge-2020.08-1.tar.bz2
RUN rm -rf riscv64--musl--bleeding-edge-2020.08-1.tar.bz2
RUN mv riscv64--musl--bleeding-edge-2020.08-1 riscv64-musl-bleeding-edge
ENV PATH=${HOME}/riscv64-musl-bleeding-edge/bin:$PATH

# 1.9. Clean up
RUN rm -rf /var/lib/apt/lists/*

# 2. Set up QEMU RISC-V and LoongArch
# - https://gitlab.educg.net/wangmingjian/os-contest-2024-image
# - https://www.qemu.org/download/
# - https://wiki.qemu.org/Documentation/Platforms/RISCV
# - https://risc-v-getting-started-guide.readthedocs.io/en/latest/linux-qemu.html

# 2.1. Download source
WORKDIR ${HOME}
RUN wget --progress=dot:giga https://download.qemu.org/qemu-${QEMU_VERSION}.tar.xz && \
    tar xvJf qemu-${QEMU_VERSION}.tar.xz

# 2.2. Build and install from source
WORKDIR ${HOME}/qemu-${QEMU_VERSION}
RUN ./configure --target-list=loongarch64-softmmu,riscv64-softmmu,riscv64-linux-user \
    --enable-gcov --enable-debug --enable-slirp && \
    make -j$(nproc) && \
    make install

# 2.3. Clean up
WORKDIR ${HOME}
RUN rm -rf qemu-${QEMU_VERSION} qemu-${QEMU_VERSION}.tar.xz

# 2.4. Sanity checking
RUN qemu-system-riscv64 --version && \
    qemu-system-loongarch64 --version && \
    qemu-riscv64 --version

# 3. Set up Rust
# - https://learningos.github.io/rust-based-os-comp2022/0setup-devel-env.html#qemu
# - https://www.rust-lang.org/tools/install
# - https://github.com/rust-lang/docker-rust/blob/master/Dockerfile-debian.template

# 3.1. Install
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=nightly-2025-01-18 \
    PROFILE=minimal
RUN set -eux; \
    wget --progress=dot:giga https://sh.rustup.rs -O rustup-init; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --profile $PROFILE --default-toolchain $RUST_VERSION; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME;

# 3.2. Sanity checking
RUN rustup --version && \
    cargo --version && \
    rustc --version

# 3.3. Add targets and components
RUN rustup target add riscv64gc-unknown-none-elf && \
    rustup target add loongarch64-unknown-none && \
    rustup target add loongarch64-unknown-linux-gnu && \
    rustup component add rust-src && \
    rustup component add rustfmt && \
    rustup component add clippy && \
    rustup component add llvm-tools && \
    cargo install cargo-binutils && \
    cargo install rustfilt

# Ready to go
WORKDIR ${HOME}
ENTRYPOINT ["tini", "--"]