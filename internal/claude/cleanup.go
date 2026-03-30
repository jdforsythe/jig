package claude

import (
	"os"
	"os/signal"
	"sync"
	"syscall"
)

// Cleanup manages temp directory cleanup on exit or signal.
type Cleanup struct {
	dirs []string
	mu   sync.Mutex
	once sync.Once
}

// NewCleanup creates a new Cleanup manager and registers signal handlers.
func NewCleanup() *Cleanup {
	c := &Cleanup{}
	c.registerSignals()
	return c
}

// Register adds a temp directory to the cleanup list.
func (c *Cleanup) Register(dir string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.dirs = append(c.dirs, dir)
}

// Run performs cleanup of all registered directories. Safe to call multiple times.
func (c *Cleanup) Run() {
	c.once.Do(func() {
		c.mu.Lock()
		dirs := make([]string, len(c.dirs))
		copy(dirs, c.dirs)
		c.mu.Unlock()

		for _, dir := range dirs {
			os.RemoveAll(dir)
		}
	})
}

func (c *Cleanup) registerSignals() {
	ch := make(chan os.Signal, 1)
	signal.Notify(ch, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-ch
		c.Run()
		os.Exit(1)
	}()
}
