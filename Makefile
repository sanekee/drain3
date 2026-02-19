
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

run-demo-match:
	cargo run --release --example drain3_demo_match

run-demo-parameters:
	cargo run --release --example drain3_demo_parameters

lint:
	cargo clippy
