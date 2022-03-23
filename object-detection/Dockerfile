FROM tensorflow/tensorflow:2.8.0

RUN apt-get update && \
    apt-get install -y clang nasm wget pkg-config libssl-dev && \ 
    apt-get -y clean

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"

RUN wget https://github.com/ryankurte/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-gnu.tgz && \
    tar zxvf cargo-binstall-x86_64-unknown-linux-gnu.tgz -C /usr/bin/ && \
    chmod +x /usr/bin/cargo-binstall

RUN cargo binstall --no-confirm cargo-watch

WORKDIR /app
COPY . .
