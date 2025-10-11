#!/bin/bash
cd "$(dirname "$0")"
cargo build --lib 2>&1 | head -100

