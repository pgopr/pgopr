# Eclipse Public License - v 2.0
#
#   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
#   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
#   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.

.PHONY: all clean install release

BINARY_NAME = pgopr

all: release

clean:
	cargo clean

build:
	cargo build

release:
	cargo build --release

install: release
	cp target/release/$(BINARY_NAME) /usr/local/bin/

uninstall:
	rm -f /usr/local/bin/$(BINARY_NAME)
