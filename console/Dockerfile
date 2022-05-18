FROM --platform=$BUILDPLATFORM ghcr.io/drogue-iot/builder:0.1.19 as builder

RUN mkdir /build
ADD . /build
WORKDIR /build

RUN npm install
RUN trunk build --release

FROM ghcr.io/drogue-iot/frontend-base:0.1.0

LABEL org.opencontainers.image.source="https://github.com/drogue-iot/drogue-ajour"

RUN mkdir /public

COPY --from=builder /build/nginx.conf /etc/nginx/nginx.conf
COPY --from=builder /build/backend.template.json /backend.template.json

RUN mkdir /endpoints
VOLUME /endpoints
COPY --from=builder /build/nginx.sh /nginx.sh
RUN chmod a+x /nginx.sh

CMD ["/nginx.sh"]

COPY --from=builder /build/dist/ /public/

EXPOSE 8080
