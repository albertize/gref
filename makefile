.PHONY: build-all clean zip-all build-local

DIST_DIR := dist
BIN := gref

build-all: clean
	@echo "Building for linux-amd64..."
	cargo build --release --target x86_64-unknown-linux-gnu
	cp target/x86_64-unknown-linux-gnu/release/$(BIN) $(DIST_DIR)/$(BIN)-linux-amd64
	@echo "Building for darwin-amd64..."
	cargo build --release --target x86_64-apple-darwin
	cp target/x86_64-apple-darwin/release/$(BIN) $(DIST_DIR)/$(BIN)-darwin-amd64
	@echo "Building for windows-amd64..."
	cargo build --release --target x86_64-pc-windows-msvc
	cp target/x86_64-pc-windows-msvc/release/$(BIN).exe $(DIST_DIR)/$(BIN)-windows-amd64.exe
	$(MAKE) zip-all

zip-all:
	cd $(DIST_DIR) && zip $(BIN)-linux-amd64.zip $(BIN)-linux-amd64
	cd $(DIST_DIR) && zip $(BIN)-darwin-amd64.zip $(BIN)-darwin-amd64
	cd $(DIST_DIR) && zip $(BIN)-windows-amd64.zip $(BIN)-windows-amd64.exe

clean:
	rm -rf $(DIST_DIR)
	mkdir -p $(DIST_DIR)

build-local:
	cargo build --release
	cp target/release/$(BIN) $(HOME)/.cargo/bin/$(BIN)
