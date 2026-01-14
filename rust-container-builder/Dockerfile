FROM alpine:latest
RUN apk update && apk add curl
COPY . /app
WORKDIR /app
CMD ["echo", "Hello, Rust container builder!"]