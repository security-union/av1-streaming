FROM docker.io/rust:1.58.1-bullseye

RUN apt-get update && \
    apt-get install -y clang nasm && \ 
    apt-get -y clean

WORKDIR /app
COPY . .
