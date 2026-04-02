// Package integration_test runs end-to-end tests by building the jig binary
// and invoking it as a subprocess with isolated HOME and working directories.
package integration_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"gopkg.in/yaml.v3"
)

// jigBinary is the path to the built jig binary, set in TestMain.
var jigBinary string

// TestMain builds the jig binary once for all integration tests.
func TestMain(m *testing.M) {
	root, err := findProjectRoot()
	if err != nil {
		fmt.Fprintf(os.Stderr, "could not find project root: %v\n", err)
		os.Exit(1)
	}

	tmpDir, err := os.MkdirTemp("", "jig-inttest-bin-*")
	if err != nil {
		fmt.Fprintf(os.Stderr, "MkdirTemp: %v\n", err)
		os.Exit(1)
	}
	defer os.RemoveAll(tmpDir)

	jigBinary = filepath.Join(tmpDir, "jig")
	build := exec.Command("go", "build", "-o", jigBinary, "./cmd/jig/")
	build.Dir = root
	build.Stdout = os.Stderr
	build.Stderr = os.Stderr
	if err := build.Run(); err != nil {
		fmt.Fprintf(os.Stderr, "build failed: %v\n", err)
		os.Exit(1)
	}

	os.Exit(m.Run())
}

// findProjectRoot walks up from the test file's directory to find go.mod.
func findProjectRoot() (string, error) {
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		return "", fmt.Errorf("runtime.Caller failed")
	}
	dir := filepath.Dir(file)
	for {
		if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
			return dir, nil
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return "", fmt.Errorf("no go.mod found")
		}
		dir = parent
	}
}

// ── test environment ──────────────────────────────────────────────────────────

// env holds isolated directories for a single test.
type env struct {
	home   string
	cwd    string
	binDir string // prepended to PATH; add fake binaries here
}

// newEnv creates fresh isolated directories for one test.
func newEnv(t *testing.T) *env {
	t.Helper()
	e := &env{
		home:   t.TempDir(),
		cwd:    t.TempDir(),
		binDir: t.TempDir(),
	}
	return e
}

// run invokes jig with the given args and returns stdout, stderr, and exit code.
func (e *env) run(args ...string) (stdout, stderr string, code int) {
	cmd := exec.Command(jigBinary, args...)
	cmd.Dir = e.cwd
	cmd.Env = append(filteredEnv(), "HOME="+e.home, "PATH="+e.binDir+string(os.PathListSeparator)+os.Getenv("PATH"))

	var outBuf, errBuf bytes.Buffer
	cmd.Stdout = &outBuf
	cmd.Stderr = &errBuf

	err := cmd.Run()
	code = 0
	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			code = exitErr.ExitCode()
		} else {
			code = 1
		}
	}
	return outBuf.String(), errBuf.String(), code
}

// filteredEnv returns the current environment with HOME removed.
func filteredEnv() []string {
	var out []string
	for _, kv := range os.Environ() {
		if !strings.HasPrefix(kv, "HOME=") {
			out = append(out, kv)
		}
	}
	return out
}

// fakeClaude writes a shell script to e.binDir/claude that responds to --version
// and passes through all other invocations without actually launching Claude.
func (e *env) fakeClaude(t *testing.T) {
	t.Helper()
	script := "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo \"1.0.0\"\n  exit 0\nfi\nexit 0\n"
	path := filepath.Join(e.binDir, "claude")
	if err := os.WriteFile(path, []byte(script), 0755); err != nil {
		t.Fatalf("writing fake claude: %v", err)
	}
}

// createProfile writes a minimal profile YAML directly into e.cwd/.jig/profiles/.
func (e *env) createProfile(t *testing.T, name, model string) {
	t.Helper()
	dir := filepath.Join(e.cwd, ".jig", "profiles")
	if err := os.MkdirAll(dir, 0755); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}
	content := fmt.Sprintf("name: %s\nmodel: %s\n", name, model)
	path := filepath.Join(dir, name+".yaml")
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
}

// ── jig init ──────────────────────────────────────────────────────────────────

func TestInit_FreshDir(t *testing.T) {
	e := newEnv(t)
	out, _, code := e.run("init")
	if code != 0 {
		t.Fatalf("jig init exited %d\nstdout: %s", code, out)
	}
	if !strings.Contains(out, "Initialized") {
		t.Errorf("output should contain 'Initialized'\ngot: %s", out)
	}
	profilesDir := filepath.Join(e.cwd, ".jig", "profiles")
	if _, err := os.Stat(profilesDir); os.IsNotExist(err) {
		t.Errorf(".jig/profiles/ not created")
	}
}

func TestInit_AlreadyInit(t *testing.T) {
	e := newEnv(t)
	e.run("init") // first time
	out, _, code := e.run("init")
	if code != 0 {
		t.Fatalf("second jig init exited %d", code)
	}
	if !strings.Contains(out, "Already initialized") {
		t.Errorf("output should contain 'Already initialized'\ngot: %s", out)
	}
}

// ── jig profiles create ───────────────────────────────────────────────────────

func TestProfilesCreate_WithFlags(t *testing.T) {
	e := newEnv(t)
	e.run("init")
	out, _, code := e.run("profiles", "create", "my-profile", "--model", "opus", "--effort", "high")
	if code != 0 {
		t.Fatalf("profiles create exited %d\nstdout: %s", code, out)
	}
	if !strings.Contains(out, "Created project profile: my-profile") {
		t.Errorf("output should confirm creation\ngot: %s", out)
	}
	path := filepath.Join(e.cwd, ".jig", "profiles", "my-profile.yaml")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Errorf("profile file not created at %s", path)
	}
}

func TestProfilesCreate_Global(t *testing.T) {
	e := newEnv(t)
	out, _, code := e.run("profiles", "create", "global-prof", "--model", "sonnet", "--global")
	if code != 0 {
		t.Fatalf("profiles create --global exited %d\nstdout: %s", code, out)
	}
	if !strings.Contains(out, "Created global profile: global-prof") {
		t.Errorf("output should confirm global creation\ngot: %s", out)
	}
	path := filepath.Join(e.home, ".jig", "profiles", "global-prof.yaml")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Errorf("global profile file not created at %s", path)
	}
}

func TestProfilesCreate_InvalidModel(t *testing.T) {
	e := newEnv(t)
	e.run("init")
	_, stderr, code := e.run("profiles", "create", "bad-prof", "--model", "gpt-4")
	if code == 0 {
		t.Error("profiles create with invalid model should exit non-zero")
	}
	if !strings.Contains(stderr, "model") && !strings.Contains(stderr, "invalid") &&
		!strings.Contains(stderr, "Error") {
		t.Errorf("stderr should mention model validation error\ngot: %s", stderr)
	}
}

func TestProfilesCreate_NoName(t *testing.T) {
	e := newEnv(t)
	_, _, code := e.run("profiles", "create")
	if code == 0 {
		t.Error("profiles create with no name should exit non-zero")
	}
}

// ── jig profiles list ─────────────────────────────────────────────────────────

func TestProfilesList_Empty(t *testing.T) {
	e := newEnv(t)
	out, _, code := e.run("profiles", "list")
	if code != 0 {
		t.Fatalf("profiles list exited %d\nstdout: %s", code, out)
	}
	if !strings.Contains(out, "No profiles found") {
		t.Errorf("output should say 'No profiles found'\ngot: %s", out)
	}
}

func TestProfilesList_WithProfiles(t *testing.T) {
	e := newEnv(t)
	e.createProfile(t, "alpha", "opus")
	e.createProfile(t, "beta", "sonnet")

	out, _, code := e.run("profiles", "list")
	if code != 0 {
		t.Fatalf("profiles list exited %d\nstdout: %s", code, out)
	}
	if !strings.Contains(out, "alpha") {
		t.Errorf("output should contain 'alpha'\ngot: %s", out)
	}
	if !strings.Contains(out, "beta") {
		t.Errorf("output should contain 'beta'\ngot: %s", out)
	}
}

func TestProfilesList_JSON(t *testing.T) {
	e := newEnv(t)
	e.createProfile(t, "json-prof", "opus")

	out, _, code := e.run("profiles", "list", "--json")
	if code != 0 {
		t.Fatalf("profiles list --json exited %d\nstdout: %s", code, out)
	}
	var profiles []map[string]interface{}
	if err := json.Unmarshal([]byte(out), &profiles); err != nil {
		t.Fatalf("output is not valid JSON: %v\nout: %s", err, out)
	}
	if len(profiles) == 0 {
		t.Error("expected at least one profile in JSON output")
	}
}

func TestProfilesList_EmptyNoJSON(t *testing.T) {
	// With no profiles, --json still shows the "no profiles" message
	// (the empty check precedes the JSON check in the implementation).
	e := newEnv(t)
	out, _, code := e.run("profiles", "list", "--json")
	if code != 0 {
		t.Fatalf("profiles list --json exited %d\nstdout: %s", code, out)
	}
	if !strings.Contains(out, "No profiles found") {
		t.Errorf("output should say 'No profiles found'\ngot: %s", out)
	}
}

// ── jig profiles show ─────────────────────────────────────────────────────────

func TestProfilesShow_Known(t *testing.T) {
	e := newEnv(t)
	e.createProfile(t, "show-me", "opus")

	out, _, code := e.run("profiles", "show", "show-me")
	if code != 0 {
		t.Fatalf("profiles show exited %d\nstdout: %s", code, out)
	}
	if !strings.Contains(out, "# Profile: show-me (resolved)") {
		t.Errorf("output should contain profile header\ngot: %s", out)
	}
	// Should be valid YAML after the comment line
	lines := strings.SplitN(out, "\n", 2)
	if len(lines) < 2 {
		t.Fatal("output too short")
	}
	var m map[string]interface{}
	if err := yaml.Unmarshal([]byte(lines[1]), &m); err != nil {
		t.Errorf("YAML portion is not valid: %v\nout: %s", err, lines[1])
	}
}

func TestProfilesShow_Unknown(t *testing.T) {
	e := newEnv(t)
	_, _, code := e.run("profiles", "show", "no-such-profile")
	if code == 0 {
		t.Error("profiles show for unknown profile should exit non-zero")
	}
}

// ── jig profiles validate ─────────────────────────────────────────────────────

func TestProfilesValidate_Valid(t *testing.T) {
	e := newEnv(t)
	e.createProfile(t, "ok-prof", "opus")

	out, _, code := e.run("profiles", "validate", "ok-prof")
	if code != 0 {
		t.Fatalf("profiles validate exited %d\nstdout: %s", code, out)
	}
	if !strings.Contains(out, `"ok-prof" is valid`) {
		t.Errorf("output should confirm validity\ngot: %s", out)
	}
}

func TestProfilesValidate_Unknown(t *testing.T) {
	e := newEnv(t)
	_, stderr, code := e.run("profiles", "validate", "ghost")
	if code == 0 {
		t.Error("profiles validate for unknown profile should exit non-zero")
	}
	if !strings.Contains(stderr, "invalid") && !strings.Contains(stderr, "not found") &&
		!strings.Contains(stderr, "Error") {
		t.Errorf("stderr should indicate failure\ngot: %s", stderr)
	}
}

// ── jig profiles export ───────────────────────────────────────────────────────

func TestProfilesExport_JSON(t *testing.T) {
	e := newEnv(t)
	e.createProfile(t, "exp-prof", "sonnet")

	out, _, code := e.run("profiles", "export", "exp-prof", "--format", "json")
	if code != 0 {
		t.Fatalf("profiles export --format json exited %d\nstdout: %s", code, out)
	}
	var m map[string]interface{}
	if err := json.Unmarshal([]byte(out), &m); err != nil {
		t.Fatalf("output is not valid JSON: %v\nout: %s", err, out)
	}
	if _, ok := m["name"]; !ok {
		t.Errorf("JSON output should contain 'name' key\ngot: %s", out)
	}
}

func TestProfilesExport_Args(t *testing.T) {
	e := newEnv(t)
	e.createProfile(t, "args-prof", "opus")

	out, _, code := e.run("profiles", "export", "args-prof", "--format", "args")
	if code != 0 {
		t.Fatalf("profiles export --format args exited %d\nstdout: %s", code, out)
	}
	if !strings.HasPrefix(strings.TrimSpace(out), "claude") {
		t.Errorf("args output should start with 'claude'\ngot: %s", out)
	}
}

func TestProfilesExport_UnknownFormat(t *testing.T) {
	e := newEnv(t)
	e.createProfile(t, "fmt-prof", "opus")

	_, _, code := e.run("profiles", "export", "fmt-prof", "--format", "toml")
	if code == 0 {
		t.Error("profiles export with unknown format should exit non-zero")
	}
}

func TestProfilesExport_UnknownProfile(t *testing.T) {
	e := newEnv(t)
	_, _, code := e.run("profiles", "export", "nope", "--format", "json")
	if code == 0 {
		t.Error("profiles export for unknown profile should exit non-zero")
	}
}

// ── jig profiles delete ───────────────────────────────────────────────────────

func TestProfilesDelete_Force(t *testing.T) {
	e := newEnv(t)
	e.createProfile(t, "bye-prof", "opus")

	out, _, code := e.run("profiles", "delete", "bye-prof", "--force")
	if code != 0 {
		t.Fatalf("profiles delete --force exited %d\nstdout: %s", code, out)
	}
	if !strings.Contains(out, "Deleted profile: bye-prof") {
		t.Errorf("output should confirm deletion\ngot: %s", out)
	}
	path := filepath.Join(e.cwd, ".jig", "profiles", "bye-prof.yaml")
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Errorf("profile file should have been deleted")
	}
}

func TestProfilesDelete_NotFound(t *testing.T) {
	e := newEnv(t)
	_, stderr, code := e.run("profiles", "delete", "ghost", "--force")
	if code == 0 {
		t.Error("profiles delete for unknown profile should exit non-zero")
	}
	if !strings.Contains(stderr, "not found") && !strings.Contains(stderr, "Error") {
		t.Errorf("stderr should indicate profile not found\ngot: %s", stderr)
	}
}

// ── jig doctor ────────────────────────────────────────────────────────────────

func TestDoctor_AllOK(t *testing.T) {
	e := newEnv(t)
	e.fakeClaude(t)
	e.run("init") // create .jig/profiles/

	out, _, code := e.run("doctor")
	if code != 0 {
		t.Fatalf("doctor exited %d\nstdout: %s", code, out)
	}
	if !strings.Contains(out, "Claude Code: OK") {
		t.Errorf("doctor should report Claude OK\ngot: %s", out)
	}
	if !strings.Contains(out, "All checks passed") {
		t.Errorf("doctor should report all checks passed\ngot: %s", out)
	}
}

func TestDoctor_NoClaude(t *testing.T) {
	e := newEnv(t)
	// No fake claude in e.binDir; use only a PATH that won't find claude
	cmd := exec.Command(jigBinary, "doctor")
	cmd.Dir = e.cwd
	cmd.Env = append(filteredEnv(), "HOME="+e.home, "PATH="+e.binDir)

	var outBuf bytes.Buffer
	cmd.Stdout = &outBuf
	cmd.Stderr = &outBuf
	cmd.Run() //nolint:errcheck — we only care about output
	out := outBuf.String()

	if !strings.Contains(out, "NOT FOUND") {
		t.Errorf("doctor should report Claude NOT FOUND\ngot: %s", out)
	}
}

// ── jig run --dry-run ─────────────────────────────────────────────────────────

func TestRunDryRun_OK(t *testing.T) {
	e := newEnv(t)
	e.fakeClaude(t)
	e.createProfile(t, "dry-prof", "opus")

	out, _, code := e.run("run", "dry-prof", "--dry-run")
	if code != 0 {
		t.Fatalf("run --dry-run exited %d\nstdout: %s", code, out)
	}
	for _, want := range []string{"Profile:", "Claude:", "Plugin dir:", "Command:"} {
		if !strings.Contains(out, want) {
			t.Errorf("dry-run output missing %q\ngot:\n%s", want, out)
		}
	}
}

func TestRunDryRun_ProfileNotFound(t *testing.T) {
	e := newEnv(t)
	e.fakeClaude(t)

	_, stderr, code := e.run("run", "no-such", "--dry-run")
	if code == 0 {
		t.Error("run --dry-run with unknown profile should exit non-zero")
	}
	combined := stderr
	if !strings.Contains(combined, "not found") && !strings.Contains(combined, "Error") {
		t.Errorf("stderr should mention profile not found\ngot: %s", combined)
	}
}

func TestRunDryRun_SuggestsSimilar(t *testing.T) {
	e := newEnv(t)
	e.fakeClaude(t)
	e.createProfile(t, "my-profile", "opus")

	// Typo: "my-profil" is close to "my-profile"
	_, stderr, code := e.run("run", "my-profil", "--dry-run")
	if code == 0 {
		t.Error("should exit non-zero for unknown profile")
	}
	if !strings.Contains(stderr, "my-profile") {
		t.Errorf("stderr should suggest 'my-profile'\ngot: %s", stderr)
	}
}

func TestRun_NoProfileNoFlag(t *testing.T) {
	e := newEnv(t)
	_, stderr, code := e.run("run")
	if code == 0 {
		t.Error("jig run with no args should exit non-zero")
	}
	if !strings.Contains(stderr, "profile name required") && !strings.Contains(stderr, "Error") {
		t.Errorf("stderr should mention profile name required\ngot: %s", stderr)
	}
}

func TestRun_TooManyArgs(t *testing.T) {
	e := newEnv(t)
	_, _, code := e.run("run", "a", "b")
	if code == 0 {
		t.Error("jig run with two positional args should exit non-zero")
	}
}
