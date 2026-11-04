# Build configuration
CC      ?= gcc
CFLAGS  := -Wall -O2 -std=c11
SRCS    = $(wildcard src/*.c)
OBJS    := $(SRCS:.c=.o)
VERSION != git describe --tags
PREFIX  ?= /usr/local
export LANG = C

.PHONY: all clean install test

all: app

app: $(OBJS)
	$(CC) $(CFLAGS) -o $@ $^ $(LDFLAGS)
	@echo "linked $@ ($(words $(OBJS)) objects)"

%.o: %.c include/%.h
	$(CC) $(CFLAGS) -c $< -o $@

ifeq ($(OS),Windows_NT)
    EXE := .exe
else
    EXE :=
endif

define HELP_TEXT
Usage: make [target]
endef

install: app
	install -d $(DESTDIR)$(PREFIX)/bin
	-cp app$(EXE) $(DESTDIR)$(PREFIX)/bin/

test: app ; ./app --self-test

clean:
	$(RM) $(OBJS) app
	for f in $$(ls *.tmp 2>/dev/null); do \
		echo "removing $$f"; mv "$$f" /tmp/; \
	done

DATE := $(shell date +%Y-%m-%d)
$(info Building $(VERSION) on $(DATE))
-include deps.mk
