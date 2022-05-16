FROM ghcr.io/drogue-iot/frontend-base:0.1.0

LABEL org.opencontainers.image.source="https://github.com/drogue-iot/drogue-ajour"

RUN mkdir /public

COPY console/nginx.conf /etc/nginx/nginx.conf
COPY console/backend.template.json /backend.template.json

RUN mkdir /endpoints
VOLUME /endpoints
COPY nginx.sh /nginx.sh
RUN chmod a+x /nginx.sh

CMD ["/nginx.sh"]

COPY console/dist/ /public/

EXPOSE 8080
