# syntax=docker/dockerfile:1.4
FROM gcr.io/distroless/cc
LABEL org.opencontainers.image.source=https://github.com/GiganticMinecraft/seichi-portal-backend

COPY --link / /work/

RUN /work/copy-platform-artifact.sh

ENTRYPOINT ["/usr/local/bin/seichi-portal-backend"]
