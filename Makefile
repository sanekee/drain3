
.PHONY: all clean

all: build

build:
	cargo build --release

clean:
	cargo clean

test:
	cargo test

run:
	cargo run --release --example drain3_demo