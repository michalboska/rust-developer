FROM rust:bullseye AS build
RUN ["mkdir", "/opt/cargo"]
WORKDIR "/opt/cargo"
COPY ex18-bin ex18-bin
COPY ex18-client ex18-client
COPY ex18-server ex18-server
COPY ex18-shared ex18-shared
COPY file.bin .
COPY Cargo.toml .
COPY Cargo.lock .
COPY Rocket.toml .
RUN ["cargo", "build", "--release"]

FROM rust:slim-bullseye AS chat
COPY --from=build /opt/cargo/target/release/ex18-bin /opt/cargo/ex18
COPY ex18-server/public /opt/cargo/ex18-server/public
COPY ex18-server/templates /opt/cargo/ex18-server/templates
COPY Rocket.toml /opt/cargo/Rocket.toml
WORKDIR /opt/cargo
ENTRYPOINT ["./ex18", "server"]