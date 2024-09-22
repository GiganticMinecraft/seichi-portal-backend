# syntax=docker/dockerfile:1.10
FROM ubuntu:22.04 as init

COPY --link / /work/

RUN case "$(uname -m)" in \
      "x86_64"  ) cp /work/artifacts/x86_64-unknown-linux-gnu/entrypoint      /seichi-portal-backend ;; \
      "armv7l"  ) cp /work/artifacts/armv7-unknown-linux-gnueabihf/entrypoint /seichi-portal-backend ;; \
      "aarch64" ) cp /work/artifacts/aarch64-unknown-linux-gnu/entrypoint     /seichi-portal-backend ;; \
      * ) exit 1 \
        ;; \
esac

RUN chmod +x /seichi-portal-backend

FROM ubuntu:22.04
LABEL org.opencontainers.image.source=https://github.com/GiganticMinecraft/seichi-portal-backend

COPY --from=init /seichi-portal-backend /seichi-portal-backend

ARG USERNAME=ubuntu
ARG GROUPNAME=ubuntu
ARG UID=1000
ARG GID=1000
RUN groupadd -g $GID $GROUPNAME && \
    useradd -m -s /bin/bash -u $UID -g $GID $USERNAME

ENTRYPOINT ["/seichi-portal-backend"]
