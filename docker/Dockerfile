# syntax=docker/dockerfile:1.23
FROM ubuntu:26.04 AS init

COPY --link / /work/

RUN case "$(uname -m)" in \
      "x86_64"  ) cp /work/artifacts/x86_64-unknown-linux-gnu/entrypoint      /seichi-portal-backend ;; \
      "armv7l"  ) cp /work/artifacts/armv7-unknown-linux-gnueabihf/entrypoint /seichi-portal-backend ;; \
      "aarch64" ) cp /work/artifacts/aarch64-unknown-linux-gnu/entrypoint     /seichi-portal-backend ;; \
      * ) exit 1 \
        ;; \
esac

RUN chmod +x /seichi-portal-backend

FROM ubuntu:26.04
LABEL org.opencontainers.image.source=https://github.com/GiganticMinecraft/seichi-portal-backend

COPY --from=init /seichi-portal-backend /seichi-portal-backend

USER ubuntu

ENTRYPOINT ["/seichi-portal-backend"]
