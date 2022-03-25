#FROM registry.access.redhat.com/ubi8-minimal
FROM registry.fedoraproject.org/fedora-minimal:35

ADD target/release/drogue-ajour /
ADD scripts/start.sh /

ENTRYPOINT [ "/start.sh" ]
