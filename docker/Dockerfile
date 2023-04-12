# syntax=docker/dockerfile:1.4
FROM gcr.io/distroless/cc
LABEL org.opencontainers.image.source=https://github.com/GiganticMinecraft/seichi-portal-backend

COPY --link / /work/

RUN case "$(uname -m)" in \
  "x86_64"  ) cp /work/artifacts/x86_64-unknown-linux-gnu/entrypoint      /usr/local/bin/seichi-portal-backend ;; \
  "armv7l"  ) cp /work/artifacts/armv7-unknown-linux-gnueabihf/entrypoint /usr/local/bin/seichi-portal-backend ;; \
  "aarch64" ) cp /work/artifacts/aarch64-unknown-linux-gnu/entrypoint     /usr/local/bin/seichi-portal-backend ;; \
  * ) exit 1 \
esac

ENTRYPOINT ["/usr/local/bin/seichi-portal-backend"]
