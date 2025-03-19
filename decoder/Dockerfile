# Dockerfile for the 2025 eCTF
# Make any changes here to set up your build environment (e.g., installing crypto
# libraries, dependencies, the compiler for a different language)

FROM rust:1.85

LABEL version="0.2"
LABEL description="Parkway Spark Decoder Builder"

ARG DEBIAN_FRONTEND=noninteractive
WORKDIR /root

# Install Requisite Packages
# do this first because it takes the longest
RUN apt-get update && apt-get upgrade -y && apt-get install -y \
    gdb \
    gdb-multiarch \
    gcc-arm-none-eabi \
    binutils-arm-none-eabi \
    python3 \
    python3-pip \
    git \
    libtool \
    texinfo \
    unzip \
    zlib1g-dev

RUN rustup target add thumbv7em-none-eabihf

COPY . /decoder
WORKDIR /
RUN pip install --no-cache-dir -r /decoder/requirements.txt --break-system-packages

CMD ["python3", "/decoder/build.py"]

# Sample run command:
# docker run --rm -v ./build_out:/out -v ./:/decoder -v ./secrets:/secrets -e DECODER_ID=0xdeadbeef decoder
