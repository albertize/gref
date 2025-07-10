package main

import (
	"fmt"
	"os"
	"regexp"

	tea "github.com/charmbracelet/bubbletea"
)

func main() {
	if len(os.Args) < 3 {
		fmt.Println("Utilizzo: go run . <directory> <pattern> [sostituzione]")
		fmt.Println("Esempio: go run . . \"vecchio_testo\" \"nuovo_testo\"")
		fmt.Println("         (Usa '.' per la directory corrente)")
		os.Exit(1)
	}
	clearScreenANSI()

	rootPath := os.Args[1]
	patternStr := os.Args[2]
	replacementStr := ""
	if len(os.Args) > 3 {
		replacementStr = os.Args[3]
	}

	// Compila il pattern regex
	pattern, err := regexp.Compile(patternStr)
	if err != nil {
		clearScreenANSI()
		fmt.Printf("Errore nella compilazione del pattern regex: %v\n", err)
		os.Exit(1)
	}

	// Esegui la ricerca iniziale
	results, err := performSearch(rootPath, pattern)
	if err != nil {
		clearScreenANSI()
		fmt.Printf("Errore durante la ricerca: %v\n", err)
		os.Exit(1)
	}

	if len(results) == 0 {
		clearScreenANSI()
		fmt.Println("Nessun risultato trovato per il pattern:", patternStr)
		os.Exit(0)
	}

	// Inizializza il modello Bubble Tea
	p := tea.NewProgram(initialModel(results, patternStr, replacementStr))

	// Avvia il programma Bubble Tea
	if _, err := p.Run(); err != nil {
		clearScreenANSI()
		fmt.Printf("Errore nell'esecuzione del programma: %v\n", err)
		os.Exit(1)
	}
}
