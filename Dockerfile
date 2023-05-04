FROM rust:1.69-buster as build

# create a new empty shell project
RUN USER=root cargo new --bin github-profile-views-counter
WORKDIR /github-profile-views-counter

# copy over manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# cache dependencies
RUN cargo build --release
RUN rm -rf ./src

# copy source files
COPY ./src ./src

# release build
RUN rm ./target/release/deps/github_profile_views_counter*
RUN cargo build --release

# our final base
FROM debian:buster-slim

# install dependencies
RUN apt-get update && apt install -y openssl && apt install -y ca-certificates
RUN update-ca-certificates


# copy the build artifact from the build stage
COPY --from=build /github-profile-views-counter/target/release/github-profile-views-counter .

# start the server
CMD ["./github-profile-views-counter"]
