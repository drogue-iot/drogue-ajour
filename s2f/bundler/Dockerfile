FROM registry.access.redhat.com/ubi8/ubi:latest

RUN dnf -y update
RUN dnf -y install git

ADD firmware/Dockerfile /
ADD scripts/bundle.sh /

CMD ["/bundle.sh"]
