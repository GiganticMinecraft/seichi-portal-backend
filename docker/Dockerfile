# syntax=docker/dockerfile:1.4
FROM ubuntu:22.04
LABEL org.opencontainers.image.source=https://github.com/GiganticMinecraft/seichi-portal-backend

COPY --link / /work/

RUN case "$(uname -m)" in \
      "x86_64"  ) cp /work/artifacts/x86_64-unknown-linux-gnu/entrypoint      /seichi-portal-backend ;; \
      "armv7l"  ) cp /work/artifacts/armv7-unknown-linux-gnueabihf/entrypoint /seichi-portal-backend ;; \
      "aarch64" ) cp /work/artifacts/aarch64-unknown-linux-gnu/entrypoint     /seichi-portal-backend ;; \
      * ) exit 1 \
        ;; \
esac

RUN adduser -D ubuntu && chown -R ubuntu /seichi-portal-backend
USER ubuntu

ENTRYPOINT ["/seichi-portal-backend"]
