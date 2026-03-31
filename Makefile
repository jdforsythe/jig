.PHONY: build install test lint clean

BIN := jig
PKG := github.com/jdforsythe/jig
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
LDFLAGS := -ldflags "-X main.version=$(VERSION)"

build:
	go build $(LDFLAGS) -o $(BIN) ./cmd/jig

install:
	go install $(LDFLAGS) ./cmd/jig

test:
	go test ./...

lint:
	golangci-lint run ./...

clean:
	rm -f $(BIN)
