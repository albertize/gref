.PHONY: build-all clean zip-all

DIST_DIR := dist
BIN := gref

build-all: clean
	@echo "Building for linux-amd64..."
	GOOS=linux GOARCH=amd64 go build -ldflags="-s -w" -o $(DIST_DIR)/$(BIN)-linux-amd64
	@echo "Building for darwin-amd64..."
	GOOS=darwin GOARCH=amd64 go build -ldflags="-s -w" -o $(DIST_DIR)/$(BIN)-darwin-amd64
	@echo "Building for windows-amd64..."
	GOOS=windows GOARCH=amd64 go build -ldflags="-s -w" -o $(DIST_DIR)/$(BIN)-windows-amd64.exe
	$(MAKE) zip-all

zip-all:
	@echo "Zipping linux-amd64..."
	cd $(DIST_DIR) && zip $(BIN)-linux-amd64.zip $(BIN)-linux-amd64
	@echo "Zipping darwin-amd64..."
	cd $(DIST_DIR) && zip $(BIN)-darwin-amd64.zip $(BIN)-darwin-amd64
	@echo "Zipping windows-amd64..."
	cd $(DIST_DIR) && zip $(BIN)-windows-amd64.zip $(BIN)-windows-amd64.exe

clean:
	rm -rf $(DIST_DIR)
	mkdir -p $(DIST_DIR)

build-local:
	@ go build -ldflags="-s -w" . && mv gref $(HOME)/go/bin
