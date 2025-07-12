package main

import (
	"fmt"
	"log"
	"net/http"
	"time"
)

// Current service version: 2.0.0
// This is a comment line for testing.
const API_URL = "https://api.example.com/v1/"
const DEFAULT_TIMEOUT = 5000 // Timeout in milliseconds

func init() {
	fmt.Println("Module initialization...")
	// Log the version at startup
	log.Printf("Service started. Version: %s\n", "2.0.0")
}

func fetchData(endpoint string) (string, error) {
	client := http.Client{
		Timeout: time.Duration(DEFAULT_TIMEOUT) * time.Millisecond,
	}

	resp, err := client.Get(API_URL + endpoint)
	if err != nil {
		return "", fmt.Errorf("Error during request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("status code non OK: %d", resp.StatusCode)
	}

	// Simuliamo la lettura del corpo
	return "Dummy data from server v1", nil
}

func main() {
	fmt.Println("Starting test application.")

	// Example usage:
	data, err := fetchData("users/123")
	if err != nil {
		log.Fatalf("Error: %v", err)
	}
	fmt.Printf("Received data: %s\n", data)

	// Another test for version 2.0.0
	// Final test v2.0.0 for verification
	const MAX_RETRIES = 3
	fmt.Println("End of application.")
}

/*
This is a multiline comment block.
It contains the version 2.0.0 multiple times.
And also API_URL and DEFAULT_TIMEOUT.
*/

//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
//API_URL
