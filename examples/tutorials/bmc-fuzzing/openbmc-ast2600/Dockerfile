FROM ubuntu:24.04 AS builder

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ENV DEBIAN_FRONTEND=noninteractive

# Standard Yocto/OpenEmbedded build-host package set for Ubuntu, per the
# Yocto Project reference manual system requirements, plus OpenBMC README's
# own minimal list.
RUN apt-get update && apt-get install -y --no-install-recommends \
    gawk wget git diffstat unzip texinfo gcc build-essential chrpath \
    socat cpio python3 python3-pip python3-pexpect python3-git \
    python3-jinja2 python3-subunit zstd liblz4-tool file locales \
    libacl1 ca-certificates bzip2 lz4 xz-utils debianutils \
    iputils-ping && \
    rm -rf /var/lib/apt/lists/*

RUN locale-gen en_US.UTF-8
ENV LANG=en_US.UTF-8 LANGUAGE=en_US:en LC_ALL=en_US.UTF-8

# bitbake refuses to run as root; build as an unprivileged user instead.
RUN useradd -m -s /bin/bash builder

ARG PROJECT=
RUN mkdir -p "$PROJECT" && chown builder:builder "$PROJECT"
WORKDIR "$PROJECT"
USER builder

ARG OPENBMC_HASH="f93f38af82e0202a4ab4fb974b689bfdc7a5a740"

RUN git clone https://github.com/openbmc/openbmc.git

RUN git -C openbmc checkout "${OPENBMC_HASH}"

WORKDIR "${PROJECT}/openbmc"

ARG MACHINE=evb-ast2600

# Drop the uart5 "A0 workaround" that switches this board's console UART
# driver to snps,dw-apb-uart: that driver probes DesignWare-specific
# extended registers the target Simics AST2600 model doesn't implement,
# which otherwise causes an unmapped write and a watchdog reboot loop before
# Linux ever reaches userspace. See the bbappend for detail.
COPY linux-aspeed_%.bbappend meta-evb/meta-evb-aspeed/meta-evb-ast2600/recipes-kernel/linux/linux-aspeed_%.bbappend

RUN bash -c 'source ./setup ${MACHINE} build && bitbake obmc-phosphor-image'

# Multi-stage: only export the final deploy/images artifacts, not the full
# multi-hundred-GB Yocto build tree (tmp/work, sstate-cache, etc.), baking
# that whole tree into a single image layer makes the image export step
# extremely slow (huge count of small files to tar up).
FROM scratch AS artifacts
COPY --from=builder /home/builder/project/openbmc/build/tmp/deploy/images/evb-ast2600/ /
