package main

import (
	"fmt"
	"regexp"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

// ANSI escape codes for coloring output
const (
	ColorRed   = lipgloss.Color("9")   // Bright Red
	ColorGreen = lipgloss.Color("10")  // Bright Green
	ColorCyan  = lipgloss.Color("6")   // Cyan
	ColorGrey  = lipgloss.Color("240") // Dark Grey
)

var (
	highlightStyle = lipgloss.NewStyle().Foreground(ColorRed)
	replaceStyle   = lipgloss.NewStyle().Foreground(ColorGreen)
	selectedStyle  = lipgloss.NewStyle().Foreground(ColorCyan).Bold(true)
	helpStyle      = lipgloss.NewStyle().Foreground(ColorGrey).Padding(0, 1)
	errorStyle     = lipgloss.NewStyle().Foreground(ColorRed).Bold(true)
)

// AppState defines the different states of our application
type AppState int

const (
	StateBrowse     AppState = iota // Browse search results
	StateConfirming                 // Confirming replacement
	StateReplacing                  // Replacing in progress (briefly)
	StateDone                       // All replacements done or user quit
)

// model represents the state of our terminal UI
type model struct {
	results        []SearchResult   // All found results
	cursor         int              // Which result is currently selected (index in m.results)
	topline        int              // Index of the first result visible on screen
	screenHeight   int              // Height of the terminal screen, used for displaying results
	selected       map[int]struct{} // Which results are marked for replacement
	patternStr     string           // The original search pattern string
	replacementStr string           // The replacement string
	state          AppState         // Current state of the application
	err            error            // Any error that occurred
}

// initialModel creates a new model with initial state
func initialModel(results []SearchResult, pattern, replacement string) model {
	return model{
		results:        results,
		cursor:         0,
		topline:        0,
		screenHeight:   20, // Default screen height, should be updated on tea.WindowSizeMsg
		selected:       make(map[int]struct{}),
		patternStr:     pattern,
		replacementStr: replacement,
		state:          StateBrowse,
	}
}

// Init is the first function that will be called. It returns an optional
// initial command.
func (m model) Init() tea.Cmd {
	return nil
}

// Update handles messages (events) and updates the model accordingly.
func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "ctrl+c", "q":
			clearScreenANSI()
			return m, tea.Quit

		case "up", "k":
			if m.state == StateBrowse {
				if m.cursor > 0 {
					m.cursor--
					// If the cursor moves above the current topline, adjust topline
					if m.cursor < m.topline {
						m.topline = m.cursor
					}
				}
			}

		case "down", "j":
			if m.state == StateBrowse {
				if m.cursor < len(m.results)-1 {
					m.cursor++
					// If the cursor moves below the current visible window, adjust topline
					if m.cursor >= m.topline+m.screenHeight {
						m.topline = m.cursor - m.screenHeight + 1
					}
				}
			}

		case " ": // Space to select/deselect an item
			if m.state == StateBrowse {
				if _, ok := m.selected[m.cursor]; ok {
					delete(m.selected, m.cursor)
				} else {
					m.selected[m.cursor] = struct{}{}
				}
			}

		case "a": // Select all
			if m.state == StateBrowse {
				for i := range m.results {
					m.selected[i] = struct{}{}
				}
			}

		case "n": // Deselect all
			if m.state == StateBrowse {
				m.selected = make(map[int]struct{})
			}

		case "enter":
			switch m.state {
			case StateBrowse:
				if len(m.selected) == 0 {
					m.err = fmt.Errorf("no results")
					return m, nil
				}
				m.state = StateConfirming
			case StateConfirming:
				m.state = StateReplacing
				// Perform replacement in a goroutine to not block the UI
				return m, func() tea.Msg {
					err := performReplacements(m.results, m.selected, m.patternStr, m.replacementStr)
					if err != nil {
						return errMsg{err}
					}
					return replacementDoneMsg{}
				}
			}

		case "esc":
			if m.state == StateConfirming {
				m.state = StateBrowse // Go back to Browse
				m.err = nil           // Clear any previous error
			}
		}

	case tea.WindowSizeMsg:
		// When the window resizes, update the screen height for pagination
		m.screenHeight = msg.Height - 10 // Adjust for header and footer
		if m.screenHeight < 1 {
			m.screenHeight = 1
		}
		// Adjust topline if necessary to keep cursor on screen
		if m.cursor < m.topline {
			m.topline = m.cursor
		}
		if m.cursor >= m.topline+m.screenHeight {
			m.topline = m.cursor - m.screenHeight + 1
		}

	case errMsg:
		m.err = msg.error
		m.state = StateDone // Stop on error
		return m, nil

	case replacementDoneMsg:
		m.state = StateDone
		return m, tea.Quit // Exit after replacement
	}

	return m, nil
}

// View renders the UI.
func (m model) View() string {
	s := strings.Builder{}

	if m.err != nil {
		s.WriteString(errorStyle.Render(fmt.Sprintf("Error: %v\n", m.err)))
		s.WriteString("\nPress 'q' to exit.\n")
		return s.String()
	}

	switch m.state {
	case StateBrowse:
		s.WriteString("--- Search results (Pattern: ")
		s.WriteString(highlightStyle.Render(m.patternStr))
		s.WriteString(") ---\n")
		if m.replacementStr != "" {
			s.WriteString("Replacing with: ")
			s.WriteString(replaceStyle.Render(m.replacementStr))
			s.WriteString("\n")
		}
		s.WriteString("\n")

		re := regexp.MustCompile(regexp.QuoteMeta(m.patternStr))

		// Calculate the end index for the visible results
		endLine := m.topline + m.screenHeight
		if endLine > len(m.results) {
			endLine = len(m.results)
		}

		// Iterate only over the visible results
		for i := m.topline; i < endLine; i++ {
			res := m.results[i] // Get the result from the full results slice using its absolute index

			cursorStr := "  "
			if m.cursor == i { // Check if the current absolute index is the cursor
				cursorStr = lipgloss.NewStyle().Bold(true).Render("> ")
			}

			checkedStr := "[ ]"
			if _, ok := m.selected[i]; ok {
				checkedStr = selectedStyle.Render("[x]")
			}

			s.WriteString(fmt.Sprintf("%s%s %s:%d: ", cursorStr, checkedStr, res.FilePath, res.LineNum))

			line := res.LineText

			baseLineTextStyle := lipgloss.NewStyle()
			if _, ok := m.selected[i]; ok {
				baseLineTextStyle = selectedStyle
			}

			matchHighlightStyle := highlightStyle
			if _, ok := m.selected[i]; ok {
				matchHighlightStyle = lipgloss.NewStyle().Foreground(ColorRed).Underline(true).Bold(true)
			}

			lastIndex := 0
			matches := re.FindAllStringIndex(line, -1)

			for _, match := range matches {
				s.WriteString(baseLineTextStyle.Render(line[lastIndex:match[0]]))
				s.WriteString(matchHighlightStyle.Render(line[match[0]:match[1]]))
				lastIndex = match[1]
			}
			s.WriteString(baseLineTextStyle.Render(line[lastIndex:]))

			s.WriteString("\n")
		}
		s.WriteString(helpStyle.Render(fmt.Sprintf("\nLine %d/%d", m.cursor+1, len(m.results))))
		s.WriteString(helpStyle.Render("\nRows/j/k: move | Space: select/deselect | a: select all | n: deselect all"))
		s.WriteString(helpStyle.Render("\nEnter: confirm selected | q/Ctrl+c: exit"))

	case StateConfirming:
		s.WriteString(fmt.Sprintf("Replacing %d?\n", len(m.selected)))
		s.WriteString(fmt.Sprintf("Pattern: %s -> Replace: %s\n\n", highlightStyle.Render(m.patternStr), replaceStyle.Render(m.replacementStr)))
		s.WriteString(helpStyle.Render("Enter: confirm | Esc: exit"))

	case StateReplacing:
		s.WriteString("Replacing... whait.\n")

	case StateDone:
		s.WriteString("Success.\n")
	}

	return s.String()
}

// CORREZIONE: Le definizioni dei messaggi erano state omesse
// Custom messages for async operations
type replacementDoneMsg struct{}
type errMsg struct{ error }

// clearScreenANSI pulisce la console usando i codici ANSI, spesso più veloci.
// Funziona meglio su terminali che supportano ANSI escape codes (quasi tutti i moderni).
func clearScreenANSI() {
	// Codice ANSI per pulire lo schermo e spostare il cursore in alto a sinistra (0;0)
	fmt.Print("\033[H\033[2J")
}
