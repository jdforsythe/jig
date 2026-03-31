# Homebrew Distribution Plan

## Goal
Ship `jig` as a Homebrew-installable binary via a personal tap (`jforsythe/jig`) using goreleaser and GitHub Actions. Target: `v0.1.0`.

---

## Prerequisites

- [ ] `github.com/jforsythe/jig` is public
- [ ] Create `github.com/jforsythe/homebrew-jig` (empty public repo — goreleaser populates it)
- [ ] Generate a GitHub PAT with `repo` scope; add as secret `HOMEBREW_TAP_GITHUB_TOKEN` in the `jig` repo

---

## Step 1 — Add `.goreleaser.yaml`

Create `.goreleaser.yaml` in the repo root:

```yaml
version: 2

before:
  hooks:
    - go mod tidy

builds:
  - id: jig
    main: ./cmd/jig
    binary: jig
    env:
      - CGO_ENABLED=0
    goos: [linux, darwin, windows]
    goarch: [amd64, arm64]
    ldflags:
      - -s -w -X main.version={{.Version}}

archives:
  - id: jig
    formats: [tar.gz]
    name_template: "{{ .ProjectName }}_{{ .Os }}_{{ .Arch }}"
    format_overrides:
      - goos: windows
        formats: [zip]

checksum:
  name_template: checksums.txt
  algorithm: sha256

changelog:
  sort: asc
  filters:
    exclude:
      - "^docs:"
      - "^test:"
      - "^chore:"

brews:
  - name: jig
    repository:
      owner: jforsythe
      name: homebrew-jig
      token: "{{ .Env.HOMEBREW_TAP_GITHUB_TOKEN }}"
    homepage: https://github.com/jforsythe/jig
    description: "Claude Code session configurator — manage profiles, MCP servers, and launch settings via a terminal UI"
    license: MIT
    install: |
      bin.install "jig"
    test: |
      system "#{bin}/jig --version"
```

**Why ldflags:** `main.version` in `cmd/jig/main.go` is a `var`, so goreleaser injects the git tag at build time. `--version` will print `jig v0.1.0` instead of `jig dev`.

---

## Step 2 — Add GitHub Actions workflow

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: actions/setup-go@v5
        with:
          go-version-file: go.mod
          cache: true

      - uses: goreleaser/goreleaser-action@v6
        with:
          version: latest
          args: release --clean
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          HOMEBREW_TAP_GITHUB_TOKEN: ${{ secrets.HOMEBREW_TAP_GITHUB_TOKEN }}
```

---

## Step 3 — Tag and release

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions runs goreleaser, which:
1. Builds 5 binaries (darwin/amd64, darwin/arm64, linux/amd64, linux/arm64, windows/amd64)
2. Creates `jig_Darwin_arm64.tar.gz` etc. and `checksums.txt`
3. Creates a GitHub Release at `github.com/jforsythe/jig/releases/tag/v0.1.0`
4. Commits a `Formula/jig.rb` to `github.com/jforsythe/homebrew-jig`

---

## Step 4 — Verify install

```bash
brew tap jforsythe/jig
brew install jig
jig --version
```

---

## User-facing install instructions (for README)

```bash
brew install jforsythe/jig/jig
```
or
```bash
brew tap jforsythe/jig
brew install jig
```

Go developers can also use:
```bash
go install github.com/jforsythe/jig@latest
```

---

## Notes

- The `HOMEBREW_TAP_GITHUB_TOKEN` PAT only needs `repo` scope on the `homebrew-jig` repo, not the main repo. The standard `GITHUB_TOKEN` handles the release assets.
- goreleaser is free for public repos (no account needed for basic use; goreleaser Pro is optional).
- Future: once the tool has real usage, submit to `homebrew/homebrew-core`. Until then the tap is the right home.
- If you add shell completions (`cobra` generates them), extend the brew formula's `install` block to install them into the Homebrew completion dirs.
