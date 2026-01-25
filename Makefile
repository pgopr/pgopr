# Eclipse Public License - v 2.0
#
#   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
#   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
#   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.

.PHONY: all clean build release install uninstall

BINARY_NAME = pgopr

all: build

clean:
	cargo clean

build:
	cargo fmt
	cargo build
	cargo clippy

release:
	cargo clean
	cargo build --release

install: release
	cp target/release/$(BINARY_NAME) /usr/local/bin/

uninstall:
	rm -f /usr/local/bin/$(BINARY_NAME)
