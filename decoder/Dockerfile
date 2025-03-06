# Dockerfile for the 2025 eCTF
# Make any changes here to set up your build environment (e.g., installing crypto
# libraries, dependencies, the compiler for a different language)

FROM rust:1.85

LABEL version="0.2"
LABEL description="Parkway Spark Decoder Builder"

ARG DEBIAN_FRONTEND=noninteractive
WORKDIR /root

# Install MSDK
RUN apt-get install -y unzip && \
    wget https://github.com/analogdevicesinc/msdk/archive/refs/tags/v2024_02.zip && \
    unzip v2024_02.zip && \
    rm -f v2024_02.zip

ENV MAXIM_PATH=/root/msdk-2024_02

# Install Requisite Packages
# do this first because it takes the longest
RUN apt-get update && apt-get upgrade -y && apt-get install -y \
    gdb \
    gdb-multiarch \
    gcc-arm-none-eabi \
    binutils-arm-none-eabi \
    make \
    python3 \
    python3-pip \
    git \
    wget \
    libtool \
    texinfo \
    unzip

RUN rustup target add thumbv7em-none-eabihf

COPY . /decoder
WORKDIR /
RUN pip install --no-cache-dir -r /decoder/requirements.txt --break-system-packages

CMD ["python3", "/decoder/build.py"]

# Sample run command:
# docker run --rm -v ./build_out:/out -v ./:/decoder -v ./secrets:/secrets -e DECODER_ID=0xdeadbeef decoder
