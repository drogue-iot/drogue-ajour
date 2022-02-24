#FROM registry.access.redhat.com/ubi8-minimal
FROM registry.fedoraproject.org/fedora-minimal:35

ADD target/release/drogue-firmware-endpoint /

ENTRYPOINT [ "/drogue-firmware-endpoint" ]
