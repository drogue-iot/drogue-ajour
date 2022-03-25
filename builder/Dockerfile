FROM registry.access.redhat.com/ubi8/ubi:latest

RUN dnf -y update
RUN dnf -y install gcc gcc-c++ git
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

ADD firmware/Dockerfile /
ADD scripts/build.sh /
ENV PATH "$PATH:/root/.cargo/bin"
ENV CARGO_HOME "/usr/src/.cargo-container-home"

RUN cargo install cargo-binutils
