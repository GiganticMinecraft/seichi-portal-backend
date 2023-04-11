# syntax=docker/dockerfile:1.4
FROM ubuntu:latest
LABEL org.opencontainers.image.source=https://github.com/GiganticMinecraft/seichi-portal-backend

COPY --link / /work/
RUN ls
COPY /seichi-portal-backend.sh /etc/service/seichi-portal-backend/run

RUN chmod +x /work/copy-platform-artifact.sh
RUN chmod +x /etc/service/seichi-portal-backend/run
RUN /work/copy-platform-artifact.sh
RUN chmod +x /usr/local/bin/seichi-portal-backend
