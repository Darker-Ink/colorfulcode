# Builds the colorfulcode static library with cargo, then links the example.

ROOT := ../..
export CARGO_TARGET_DIR ?= $(ROOT)/target
LIB := $(CARGO_TARGET_DIR)/release/libcolorfulcode_ffi.a

CFLAGS ?= -O2 -std=c99 -Wall -Wextra -Wpedantic
CPPFLAGS += -I$(ROOT)/crates/colorfulcode-ffi/include
ifeq ($(shell uname -s),Linux)
LDLIBS += -lpthread -ldl -lm
endif

highlight: main.c $(LIB)
	$(CC) $(CFLAGS) $(CPPFLAGS) -o $@ main.c $(LIB) $(LDLIBS)

$(LIB): FORCE
	cargo build --release -p colorfulcode-ffi --manifest-path $(ROOT)/Cargo.toml

run: highlight
	./highlight main.c

clean:
	rm -f highlight

.PHONY: run clean FORCE
