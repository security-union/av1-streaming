FROM docker.io/rust:1.58.1-bullseye

RUN wget https://github.com/ryankurte/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-gnu.tgz && \
    tar zxvf cargo-binstall-x86_64-unknown-linux-gnu.tgz -C /usr/bin/ && \
    chmod +x /usr/bin/cargo-binstall

RUN cargo binstall --no-confirm cargo-watch

RUN apt-get update && \
    apt-get install -y clang nasm && \ 
    apt-get -y clean

WORKDIR /app
COPY . .
