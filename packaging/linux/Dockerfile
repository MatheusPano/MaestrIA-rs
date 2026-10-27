FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl git pkg-config build-essential clang cmake \
      libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev \
      libx11-dev libx11-xcb-dev libxcb1-dev libxi-dev libxcursor-dev libxrandr-dev \
      libfontconfig-dev libfreetype-dev libssl-dev libzstd-dev libasound2-dev \
      dpkg-dev fakeroot \
    && rm -rf /var/lib/apt/lists/*

ARG RUST_VERSION=1.93.0
ENV RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:${PATH}
RUN curl -fsSL https://sh.rustup.rs | CARGO_HOME=/usr/local/cargo sh -s -- -y --profile minimal \
      --default-toolchain "${RUST_VERSION}" --no-modify-path \
    && CARGO_HOME=/usr/local/cargo cargo install cargo-deb --locked \
    && rm -rf /usr/local/cargo/registry /usr/local/cargo/git \
    && chmod -R a+rX /usr/local/cargo /usr/local/rustup

COPY make-deb.sh /usr/local/bin/make-deb
RUN chmod +x /usr/local/bin/make-deb

ENTRYPOINT ["/usr/local/bin/make-deb"]
