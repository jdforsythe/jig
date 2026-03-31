# demo.tape — generate with: vhs demo.tape
# Requires: vhs (https://github.com/charmbracelet/vhs)
#   brew install vhs
#   vhs demo.tape
#
# NOTE: switchScreen() rebuilds HomeModel fresh each time, resetting cursor to 0.
# Navigation after any screen return must account for this.
#
# Profile load order (project first, then global, each alphabetically):
#   obsidian(0)  prose(1)  fullstack(2)  quick(3)  research(4)

Output demo.gif

Set FontSize 14
Set Width 1200
Set Height 700
Set Theme "Catppuccin Mocha"
Set PlaybackSpeed 1.0
Set TypingSpeed 80ms

Env PATH "/Users/jforsythe/go/bin:/Users/jforsythe/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

# Launch jig TUI
Type "jig"
Enter
Sleep 2s

# Preview obsidian (already at index 0) — shows MCP server in YAML
Type "v"
Sleep 2s
Down@300ms
Down@300ms
Sleep 600ms
Type "q"
Sleep 700ms

# Cursor resets to 0 (obsidian) — scroll down through all profiles to show the list
Down@350ms
Down@350ms
Down@350ms
Down@350ms
Sleep 500ms

# Navigate back up to fullstack (index 2)
Up@300ms
Up@300ms
Sleep 500ms

# Edit fullstack (now at index 2)
Type "e"
Sleep 2s

# Tools tab — shows allowed/disallowed tools
Tab
Sleep 1s

# MCP Servers tab
Tab
Sleep 600ms

# Hooks tab
Tab
Sleep 600ms

# Components tab
Tab
Sleep 800ms

# Filter for web-developer agent
Type "/"
Sleep 400ms
Type "web"
Sleep 900ms

# Exit filter mode (cursor on web-developer)
Enter
Sleep 400ms

# Toggle web-developer selected
Type " "
Sleep 500ms

# Save and return to home
Type "s"
Sleep 1s

# Quit the TUI (cursor reset to 0 on return, doesn't matter — we're quitting)
Type "q"
Sleep 800ms

# Show profile list via CLI
Type "jig profiles list"
Enter
Sleep 1.5s

# Run doctor
Type "jig doctor"
Enter
Sleep 2s
