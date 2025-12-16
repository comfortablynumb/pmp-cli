//! Rollback manager for import operations
//!
//! Tracks created files and directories during import, allowing cleanup on failure.

use std::path::PathBuf;

use crate::traits::FileSystem;

/// Tracks files and directories created during import for rollback
pub struct RollbackManager {
    /// Files created during the import (in creation order)
    created_files: Vec<PathBuf>,
    /// Directories created during the import (in creation order)
    created_dirs: Vec<PathBuf>,
}

impl Default for RollbackManager {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl RollbackManager {
    /// Create a new rollback manager
    pub fn new() -> Self {
        Self {
            created_files: Vec::new(),
            created_dirs: Vec::new(),
        }
    }

    /// Track a file that was created
    pub fn track_file(&mut self, path: PathBuf) {
        self.created_files.push(path);
    }

    /// Track a directory that was created
    pub fn track_dir(&mut self, path: PathBuf) {
        self.created_dirs.push(path);
    }

    /// Get the number of tracked items
    pub fn tracked_count(&self) -> usize {
        self.created_files.len() + self.created_dirs.len()
    }

    /// Check if any items are tracked
    pub fn has_tracked_items(&self) -> bool {
        !self.created_files.is_empty() || !self.created_dirs.is_empty()
    }

    /// Roll back all tracked changes
    ///
    /// Deletes files first (in reverse order), then directories (in reverse order).
    /// Errors during rollback are silently ignored to ensure best-effort cleanup.
    pub fn rollback(&self, fs: &dyn FileSystem) -> RollbackResult {
        let mut result = RollbackResult::default();

        // Delete files first (in reverse order of creation)
        for path in self.created_files.iter().rev() {
            if fs.exists(path) {
                match fs.remove_file(path) {
                    Ok(()) => result.files_removed += 1,
                    Err(_) => result.files_failed += 1,
                }
            }
        }

        // Then delete directories (in reverse order)
        // This ensures child directories are deleted before parents
        for path in self.created_dirs.iter().rev() {
            if fs.exists(path) {
                match fs.remove_dir_all(path) {
                    Ok(()) => result.dirs_removed += 1,
                    Err(_) => result.dirs_failed += 1,
                }
            }
        }

        result
    }

    /// Clear all tracked items without rolling back
    ///
    /// Call this after a successful import to prevent accidental rollback.
    pub fn clear(&mut self) {
        self.created_files.clear();
        self.created_dirs.clear();
    }

    /// Get list of tracked files (for debugging/logging)
    pub fn tracked_files(&self) -> &[PathBuf] {
        &self.created_files
    }

    /// Get list of tracked directories (for debugging/logging)
    pub fn tracked_dirs(&self) -> &[PathBuf] {
        &self.created_dirs
    }
}

/// Result of a rollback operation
#[derive(Debug, Default)]
pub struct RollbackResult {
    /// Number of files successfully removed
    pub files_removed: usize,
    /// Number of files that failed to remove
    pub files_failed: usize,
    /// Number of directories successfully removed
    pub dirs_removed: usize,
    /// Number of directories that failed to remove
    pub dirs_failed: usize,
}

#[allow(dead_code)]
impl RollbackResult {
    /// Check if the rollback was fully successful
    pub fn is_complete(&self) -> bool {
        self.files_failed == 0 && self.dirs_failed == 0
    }

    /// Get total items removed
    pub fn total_removed(&self) -> usize {
        self.files_removed + self.dirs_removed
    }

    /// Get total items that failed
    pub fn total_failed(&self) -> usize {
        self.files_failed + self.dirs_failed
    }
}

impl std::fmt::Display for RollbackResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_complete() {
            write!(
                f,
                "Rollback complete: removed {} files and {} directories",
                self.files_removed, self.dirs_removed
            )
        } else {
            write!(
                f,
                "Rollback partial: removed {}/{} files, {}/{} directories",
                self.files_removed,
                self.files_removed + self.files_failed,
                self.dirs_removed,
                self.dirs_removed + self.dirs_failed
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::MockFileSystem;

    #[test]
    fn test_rollback_manager_new() {
        let manager = RollbackManager::new();
        assert_eq!(manager.tracked_count(), 0);
        assert!(!manager.has_tracked_items());
    }

    #[test]
    fn test_track_items() {
        let mut manager = RollbackManager::new();

        manager.track_file(PathBuf::from("/test/file.txt"));
        manager.track_dir(PathBuf::from("/test/dir"));

        assert_eq!(manager.tracked_count(), 2);
        assert!(manager.has_tracked_items());
        assert_eq!(manager.tracked_files().len(), 1);
        assert_eq!(manager.tracked_dirs().len(), 1);
    }

    #[test]
    fn test_clear() {
        let mut manager = RollbackManager::new();
        manager.track_file(PathBuf::from("/test/file.txt"));
        manager.track_dir(PathBuf::from("/test/dir"));

        manager.clear();

        assert_eq!(manager.tracked_count(), 0);
        assert!(!manager.has_tracked_items());
    }

    #[test]
    fn test_rollback() {
        let mut manager = RollbackManager::new();
        let file1 = PathBuf::from("/test/file1.txt");
        let file2 = PathBuf::from("/test/file2.txt");
        let subdir = PathBuf::from("/test/subdir");
        let testdir = PathBuf::from("/test");

        manager.track_file(file1.clone());
        manager.track_file(file2.clone());
        manager.track_dir(subdir.clone());
        manager.track_dir(testdir.clone());

        // Create mock filesystem with files and directories
        let fs = MockFileSystem::new();
        fs.write(&file1, "content1").unwrap();
        fs.write(&file2, "content2").unwrap();
        fs.create_dir_all(&subdir).unwrap();
        fs.create_dir_all(&testdir).unwrap();

        let result = manager.rollback(&fs);

        assert!(result.is_complete());
        assert_eq!(result.files_removed, 2);
        assert_eq!(result.dirs_removed, 2);

        // Verify files and dirs are gone
        assert!(!fs.exists(&file1));
        assert!(!fs.exists(&file2));
    }

    #[test]
    fn test_rollback_result_display() {
        let complete = RollbackResult {
            files_removed: 3,
            files_failed: 0,
            dirs_removed: 2,
            dirs_failed: 0,
        };
        assert!(complete.to_string().contains("complete"));

        let partial = RollbackResult {
            files_removed: 2,
            files_failed: 1,
            dirs_removed: 1,
            dirs_failed: 1,
        };
        assert!(partial.to_string().contains("partial"));
    }
}
