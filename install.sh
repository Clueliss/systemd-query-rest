#!/bin/bash

set -e

cargo build --release
install target/release/systemd-query-rest /usr/local/bin
install deploy/systemd-query-rest.service /etc/systemd/system