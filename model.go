package main

import (
	"fmt"
	"regexp"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

// ANSI escape codes for coloring terminal output
const (
	ColorRed   = lipgloss.Color("9")   // Bright red for highlights
	ColorGreen = lipgloss.Color("10")  // Bright green for replacements
	ColorCyan  = lipgloss.Color("6")   // Cyan for selection
	ColorGrey  = lipgloss.Color("240") // Dark grey for help and less important text
)

var (
	highlightStyle = lipgloss.NewStyle().Foreground(ColorRed)
	replaceStyle   = lipgloss.NewStyle().Foreground(ColorGreen)
	selectedStyle  = lipgloss.NewStyle().Foreground(ColorCyan).Bold(true)
	helpStyle      = lipgloss.NewStyle().Foreground(ColorGrey).Padding(0, 1)
	errorStyle     = lipgloss.NewStyle().Foreground(ColorRed).Bold(true)
)

// AppState represents the different UI states of the application
type AppState int

const (
	StateBrowse     AppState = iota // User is browsing search results
	StateConfirming                 // User is confirming replacements
	StateReplacing                  // Replacements are being performed
	StateDone                       // All replacements are done or user quit
)

// AppMode represents the different modes the application provides
type AppMode int

const (
	Default AppMode = iota
	SearchOnly
)

// model holds the state of the terminal UI
type model struct {
	results          []SearchResult   // All search results found
	cursor           int              // Index of the currently selected result
	topline          int              // Index of the first visible result on screen
	screenHeight     int              // Height of the terminal screen for displaying results
	screenWidth      int              // Width of the terminal screen for displaying results
	selected         map[int]struct{} // Indices of results marked for replacement
	pattern          *regexp.Regexp   // The search pattern string
	patternStr       string           // The search pattern string
	replacementStr   string           // The replacement string
	mode             AppMode          // The current Application mode
	state            AppState         // Current UI state
	err              error            // Any error that occurred
	horizontalOffset int              // Horizontal scroll offset for long lines
}

// initialModel returns a new model with the initial state
func initialModel(results []SearchResult, patternStr, replacement string, pattern *regexp.Regexp, mode AppMode) model {
	return model{
		results:          results,
		cursor:           0,
		topline:          0,
		screenHeight:     20, // Default screen height, should be updated on tea.WindowSizeMsg
		screenWidth:      20, // Default screen width, should be updated on tea.WindowSizeMsg
		selected:         make(map[int]struct{}),
		pattern:          pattern,
		patternStr:       patternStr,
		replacementStr:   replacement,
		mode:             mode,
		state:            StateBrowse,
		horizontalOffset: 0,
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
			return m, tea.Quit

		case "up", "k":
			if m.state == StateBrowse {
				if m.cursor > 0 {
					m.cursor--
					if m.cursor < m.topline {
						m.topline = m.cursor
					}
				}
			}

		case "down", "j":
			if m.state == StateBrowse {
				if m.cursor < len(m.results)-1 {
					m.cursor++
					if m.cursor >= m.topline+m.screenHeight {
						m.topline = m.cursor - m.screenHeight + 1
					}
				}
			}

		case "left", "h":
			if m.state == StateBrowse && m.horizontalOffset > 0 {
				m.horizontalOffset -= 10
				if m.horizontalOffset < 0 {
					m.horizontalOffset = 0
				}
			}

		case "right", "l":
			if m.state == StateBrowse {
				// Calculate available width for line text (excluding decorations)
				// Decorations: cursorStr (2), checkedStr (4), filePath, colon, lineNum, spaces, etc.
				// We'll estimate decorations as: len(filePath) + len(":") + len(lineNum) + 6
				availableWidth := m.screenWidth - 20 // 20 is a safe estimate for decorations
				if availableWidth < 1 {
					availableWidth = 1
				}
				maxOffset := 0
				endLine := min(m.topline+m.screenHeight, len(m.results))
				for i := m.topline; i < endLine; i++ {
					lineLen := len(m.results[i].LineText)
					offset := lineLen - availableWidth
					if offset > maxOffset {
						maxOffset = offset
					}
				}
				if maxOffset < 0 {
					maxOffset = 0
				}
				m.horizontalOffset += 5
				if m.horizontalOffset > maxOffset {
					m.horizontalOffset = maxOffset
				}
			}

		case "home":
			if m.state == StateBrowse {
				m.horizontalOffset = 0
			}

		case "end":
			if m.state == StateBrowse {
				m.horizontalOffset = 1000 // Arbitrary large value to scroll to end
			}

		case " ": // Space to select/deselect an item
			if m.state == StateBrowse && m.mode != SearchOnly {
				if _, ok := m.selected[m.cursor]; ok {
					delete(m.selected, m.cursor)
				} else {
					m.selected[m.cursor] = struct{}{}
				}
			}

		case "a": // Select all
			if m.state == StateBrowse && m.mode != SearchOnly {
				for i := range m.results {
					m.selected[i] = struct{}{}
				}
			}

		case "n": // Deselect all
			if m.state == StateBrowse && m.mode != SearchOnly {
				m.selected = make(map[int]struct{})
			}

		case "enter":
			switch m.state {
			case StateBrowse:
				if m.mode != SearchOnly {
					if len(m.selected) == 0 {
						m.err = fmt.Errorf("no results")
						return m, nil
					}
					m.state = StateConfirming
				}
			case StateConfirming:
				m.state = StateReplacing
				// Perform replacement in a goroutine to not block the UI
				return m, func() tea.Msg {
					err := performReplacements(m.results, m.selected, m.pattern, m.replacementStr)
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

		m.screenWidth = max(msg.Width, 1)
		// Adjust for header and footer
		m.screenHeight = max(msg.Height-10, 1)

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
	s.WriteString(m.headerView())

	if m.state == StateBrowse {
		re := regexp.MustCompile(regexp.QuoteMeta(m.patternStr))
		linesShown := 0
		// Build a mapping from visible line index to result index
		visibleLines := make([]struct {
			isHeader bool
			file     string
			idx      int
		}, 0, len(m.results)*2)
		for i, res := range m.results {
			displayPath := res.FilePath
			if i == 0 || displayPath != m.results[i-1].FilePath {
				visibleLines = append(visibleLines, struct {
					isHeader bool
					file     string
					idx      int
				}{true, displayPath, -1})
			}
			visibleLines = append(visibleLines, struct {
				isHeader bool
				file     string
				idx      int
			}{false, displayPath, i})
		}
		// Ensure cursor is always visible
		cursorLine := 0
		for i := 0; i < len(visibleLines); i++ {
			if !visibleLines[i].isHeader && visibleLines[i].idx == m.cursor {
				cursorLine = i
				break
			}
		}
		// Adjust topline if cursor is out of view
		if cursorLine < m.topline {
			m.topline = cursorLine
		}
		if cursorLine >= m.topline+m.screenHeight {
			m.topline = cursorLine - m.screenHeight + 1
		}
		// Render only visible lines
		for i := m.topline; i < len(visibleLines) && linesShown < m.screenHeight; i++ {
			v := visibleLines[i]
			if v.isHeader {
				s.WriteString(fmt.Sprintf("/ %s\n", v.file))
			} else {
				res := m.results[v.idx]
				cursorStr := "  "
				if m.cursor == v.idx {
					cursorStr = lipgloss.NewStyle().Bold(true).Render("> ")
				}
				checkedStr := "[ ]"
				if _, ok := m.selected[v.idx]; ok {
					checkedStr = selectedStyle.Render("[x]")
				}
				line := res.LineText
				baseLineTextStyle := lipgloss.NewStyle()
				visibleLine := line
				if m.horizontalOffset < len(line) {
					visibleLine = line[m.horizontalOffset:]
				} else {
					visibleLine = ""
				}
				lastIndex := 0
				matches := re.FindAllStringIndex(visibleLine, -1)
				s.WriteString(fmt.Sprintf("%s%s %d: ", cursorStr, checkedStr, res.LineNum))
				for _, match := range matches {
					s.WriteString(baseLineTextStyle.Render(visibleLine[lastIndex:match[0]]))
					if _, ok := m.selected[v.idx]; ok {
						s.WriteString(selectedStyle.Render(m.replacementStr))
					} else {
						s.WriteString(highlightStyle.Render(visibleLine[match[0]:match[1]]))
					}
					lastIndex = match[1]
				}
				s.WriteString(baseLineTextStyle.Render(visibleLine[lastIndex:]))
				s.WriteString("\n")
			}
			linesShown++
		}
	}
	s.WriteString(m.footerView())
	return s.String()
}

// Custom messages for async operations
type replacementDoneMsg struct{}
type errMsg struct{ error }

// headerView returns the header string used in the View
func (m model) headerView() string {
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
		switch m.mode {
		case SearchOnly:
			s.WriteString("Search Only Mode\n")
		default:
			s.WriteString("Replacing with: ")
			s.WriteString(replaceStyle.Render(m.replacementStr))
			s.WriteString("\n")
		}
		s.WriteString("\n")
	case StateConfirming:
		s.WriteString(fmt.Sprintf("Replacing %d?\n", len(m.selected)))
		s.WriteString(fmt.Sprintf("Pattern: %s -> Replace: %s\n\n", highlightStyle.Render(m.patternStr), replaceStyle.Render(m.replacementStr)))
	case StateReplacing:
		s.WriteString("Replacing... whait.\n")
	case StateDone:
		s.WriteString("Success.\n")
	}
	return s.String()
}

// footerView returns the footer string used in the View
func (m model) footerView() string {
	s := strings.Builder{}
	switch m.state {
	case StateBrowse:
		s.WriteString(helpStyle.Render(fmt.Sprintf("\nLine %d/%d", m.cursor+1, len(m.results))))
		s.WriteString(helpStyle.Render("\nup/down /j/k: move | left/right /h/l: scroll horizontally | Home/End: scroll to start/end of line \nSpace: select/deselect | a: select all | n: deselect all"))
	case StateConfirming:
		s.WriteString(helpStyle.Render("Enter: confirm | Esc: exit"))
	}
	return s.String()
}
