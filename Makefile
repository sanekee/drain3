
.PHONY: all clean

all: build

build:
	cargo build --release

clean:
	cargo clean

test:
	cargo test

run-demo:
	cargo run --release --example drain3_demo

lint:
	cargo clippy
