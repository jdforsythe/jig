/// Utilities for detecting git worktree context.
///
/// A git worktree has `.git` as a file containing `gitdir: <path>`,
/// while the main checkout has `.git` as a directory.
use std::path::{Path, PathBuf};

/// Returns true if `dir` is inside a git worktree (not the main checkout).
pub fn is_git_worktree(dir: &Path) -> bool {
    dir.join(".git").is_file()
}

/// Returns the main checkout root if `dir` is a worktree, otherwise `None`.
///
/// Reads `.git` (the gitdir file), parses the `gitdir:` path, and navigates
/// from `.git/worktrees/<name>` up two levels to reach the main `.git` dir,
/// then one more to reach the main working tree root.
pub fn main_worktree_path(dir: &Path) -> Option<PathBuf> {
    let git_file = dir.join(".git");
    if !git_file.is_file() {
        return None;
    }
    let contents = std::fs::read_to_string(&git_file).ok()?;
    // Expected format: "gitdir: /path/to/.git/worktrees/<name>\n"
    let gitdir_str = contents.strip_prefix("gitdir: ")?.trim();
    let gitdir = Path::new(gitdir_str);
    // gitdir = .git/worktrees/<name>  →  parent = .git/worktrees  →  parent = .git
    let main_git = gitdir.parent()?.parent()?;
    // main working tree = parent of .git
    main_git.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_git_worktree_false_for_main_checkout() {
        let dir = tempfile::tempdir().unwrap();
        // Create a .git directory (main checkout)
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        assert!(!is_git_worktree(dir.path()), ".git as directory is not a worktree");
    }

    #[test]
    fn test_is_git_worktree_true_for_gitdir_file() {
        let dir = tempfile::tempdir().unwrap();
        // Create .git as a file (worktree link)
        std::fs::write(
            dir.path().join(".git"),
            "gitdir: /some/path/.git/worktrees/my-wt\n",
        )
        .unwrap();
        assert!(is_git_worktree(dir.path()), ".git as file is a worktree");
    }

    #[test]
    fn test_is_git_worktree_false_for_no_git() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_git_worktree(dir.path()), "no .git means not a worktree");
    }

    #[test]
    fn test_main_worktree_path_parses_gitdir() {
        let dir = tempfile::tempdir().unwrap();
        // Simulate a worktree where the main tree is at /projects/myrepo
        // gitdir file: gitdir: /projects/myrepo/.git/worktrees/feature
        let fake_main = tempfile::tempdir().unwrap();
        let fake_git = fake_main.path().join(".git");
        std::fs::create_dir(&fake_git).unwrap();
        let fake_worktrees = fake_git.join("worktrees");
        std::fs::create_dir(&fake_worktrees).unwrap();
        let fake_wt = fake_worktrees.join("my-wt");
        std::fs::create_dir(&fake_wt).unwrap();

        let gitdir_content = format!("gitdir: {}\n", fake_wt.display());
        std::fs::write(dir.path().join(".git"), &gitdir_content).unwrap();

        let main = main_worktree_path(dir.path());
        assert!(main.is_some(), "should parse main worktree path");
        // The result should be the parent of fake_git = fake_main.path()
        let expected = fake_main.path().to_path_buf();
        assert_eq!(main.unwrap(), expected);
    }
}
