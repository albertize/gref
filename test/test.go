package main

import (
	"fmt"
	"log"
	"net/http"
	"time"
)

// Versione corrente del servizio: 2.0.0
// Questa è una riga di commento per test.
const API_URL = "https://api.example.com/v1/"
const DEFAULT_TIMEOUT = 5000 // Timeout in millisecondi

func init() {
	fmt.Println("Inizializzazione del modulo...")
	// Logga la versione all'avvio
	log.Printf("Servizio avviato. Versione: %s\n", "2.0.0")
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
	return "Dati fittizi dal server v1", nil
}

func main() {
	fmt.Println("Avvio dell'applicazione di test.")

	// Esempio di utilizzo:
	data, err := fetchData("users/123")
	if err != nil {
		log.Fatalf("Error: %v", err)
	}
	fmt.Printf("Dati ricevuti: %s\n", data)

	// Un altro test sulla versione 2.0.0
	// Test finale v2.0.0 per verifica
	const MAX_RETRIES = 3
	fmt.Println("Fine dell'applicazione.")
}

/*
Questo è un blocco di commento multilinea.
Contiene la versione 2.0.0 più volte.
E anche API_URL e DEFAULT_TIMEOUT.
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
