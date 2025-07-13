.PHONY: build-all clean

# Output directory
DIST_DIR := dist

# Binary name
BIN := gref

build-all: clean
	@echo "Building for linux-amd64..."
	GOOS=linux GOARCH=amd64 go build -o $(DIST_DIR)/$(BIN)-linux-amd64
	@echo "Building for darwin-amd64..."
	GOOS=darwin GOARCH=amd64 go build -o $(DIST_DIR)/$(BIN)-darwin-amd64
	@echo "Building for windows-amd64..."
	GOOS=windows GOARCH=amd64 go build -o $(DIST_DIR)/$(BIN)-windows-amd64.exe

clean:
	rm -rf $(DIST_DIR)
	mkdir -p $(DIST_DIR)

build-local:
	@ go build -ldflags="-s -w" . && mv gref $(HOME)/go/bin
