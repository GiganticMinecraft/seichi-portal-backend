# syntax=docker/dockerfile:1
# バイナリは CI 側で cross によりクロスコンパイル済みのものを artifacts/ から受け取る。
# ここでビルドすると arm64 が QEMU エミュレーションになり 1 時間以上かかる。
FROM --platform=$BUILDPLATFORM ubuntu:26.04 AS init

ARG TARGETARCH
COPY --link artifacts/ /artifacts/

RUN case "$TARGETARCH" in \
      "amd64" ) cp /artifacts/x86_64-unknown-linux-gnu/redmine-importer  /redmine-importer ;; \
      "arm64" ) cp /artifacts/aarch64-unknown-linux-gnu/redmine-importer /redmine-importer ;; \
      * ) exit 1 \
        ;; \
esac

RUN chmod +x /redmine-importer

FROM ubuntu:26.04
LABEL org.opencontainers.image.source=https://github.com/GiganticMinecraft/seichi-portal-backend

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=init /redmine-importer /redmine-importer
COPY server/redmine-importer/config/redmine-import.json /etc/seichi-portal/redmine-import.json

ENV REDMINE_IMPORT_CONFIG=/etc/seichi-portal/redmine-import.json

USER ubuntu

ENTRYPOINT ["/redmine-importer"]
