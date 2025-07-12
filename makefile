default:
	@ go build -ldflags="-s -w" . && mv gref $(HOME)/go/bin
