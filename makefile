default:
	@ go build -ldflags="-s -w" . && mv gref /home/alberto/go/bin
