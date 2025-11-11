use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Trait for filesystem operations to enable testing with mocks
pub trait FileSystem: Send + Sync {
    /// Read file contents as string
    fn read_to_string(&self, path: &Path) -> Result<String>;

    /// Write string contents to file
    fn write(&self, path: &Path, contents: &str) -> Result<()>;

    /// Create directory and all parent directories
    fn create_dir_all(&self, path: &Path) -> Result<()>;

    /// Remove directory and all its contents
    #[allow(dead_code)]
    fn remove_dir_all(&self, path: &Path) -> Result<()>;

    /// Remove a file
    #[allow(dead_code)]
    fn remove_file(&self, path: &Path) -> Result<()>;

    /// Check if path exists
    fn exists(&self, path: &Path) -> bool;

    /// Check if path is a directory
    fn is_dir(&self, path: &Path) -> bool;

    /// Check if path is a file
    fn is_file(&self, path: &Path) -> bool;

    /// Read directory entries
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>>;

    /// Walk directory recursively (for template discovery)
    fn walk_dir(&self, path: &Path, max_depth: usize) -> Result<Vec<PathBuf>>;
}

/// Real filesystem implementation using std::fs
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn read_to_string(&self, path: &Path) -> Result<String> {
        std::fs::read_to_string(path).with_context(|| format!("Failed to read file: {:?}", path))
    }

    fn write(&self, path: &Path, contents: &str) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directory: {:?}", parent))?;
        }

        std::fs::write(path, contents).with_context(|| format!("Failed to write file: {:?}", path))
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)
            .with_context(|| format!("Failed to create directory: {:?}", path))
    }

    fn remove_dir_all(&self, path: &Path) -> Result<()> {
        std::fs::remove_dir_all(path)
            .with_context(|| format!("Failed to remove directory: {:?}", path))
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        std::fs::remove_file(path).with_context(|| format!("Failed to remove file: {:?}", path))
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let entries = std::fs::read_dir(path)
            .with_context(|| format!("Failed to read directory: {:?}", path))?;

        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            paths.push(entry.path());
        }

        Ok(paths)
    }

    fn walk_dir(&self, path: &Path, max_depth: usize) -> Result<Vec<PathBuf>> {
        use walkdir::WalkDir;

        let mut paths = Vec::new();
        for entry in WalkDir::new(path).max_depth(max_depth) {
            let entry = entry.context("Failed to walk directory")?;
            paths.push(entry.path().to_path_buf());
        }

        Ok(paths)
    }
}

/// Mock filesystem implementation for testing (in-memory)
#[allow(dead_code)]
pub struct MockFileSystem {
    files: Arc<RwLock<HashMap<PathBuf, String>>>,
    directories: Arc<RwLock<HashMap<PathBuf, ()>>>,
}

#[allow(dead_code)]
impl MockFileSystem {
    /// Create new empty mock filesystem
    pub fn new() -> Self {
        Self {
            files: Arc::new(RwLock::new(HashMap::new())),
            directories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get captured file contents for testing assertions
    pub fn get_file_contents(&self, path: &Path) -> Option<String> {
        self.files.read().unwrap().get(path).cloned()
    }

    /// Check if file was written
    pub fn has_file(&self, path: &Path) -> bool {
        self.files.read().unwrap().contains_key(path)
    }

    /// List all files in mock filesystem
    pub fn list_files(&self) -> Vec<PathBuf> {
        self.files.read().unwrap().keys().cloned().collect()
    }
}

impl Default for MockFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystem for MockFileSystem {
    fn read_to_string(&self, path: &Path) -> Result<String> {
        self.files
            .read()
            .unwrap()
            .get(path)
            .cloned()
            .with_context(|| format!("File not found in mock filesystem: {:?}", path))
    }

    fn write(&self, path: &Path, contents: &str) -> Result<()> {
        // Ensure all parent directories exist in mock (recursively)
        if let Some(parent) = path.parent() {
            self.create_dir_all(parent)?;
        }

        self.files
            .write()
            .unwrap()
            .insert(path.to_path_buf(), contents.to_string());
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        self.directories
            .write()
            .unwrap()
            .insert(path.to_path_buf(), ());

        // Also add parent directories
        let mut current = path;
        while let Some(parent) = current.parent() {
            self.directories
                .write()
                .unwrap()
                .insert(parent.to_path_buf(), ());
            current = parent;
        }

        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> Result<()> {
        // Remove directory
        self.directories.write().unwrap().remove(path);

        // Remove all files in this directory
        let mut files = self.files.write().unwrap();
        files.retain(|file_path, _| !file_path.starts_with(path));

        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        self.files
            .write()
            .unwrap()
            .remove(path)
            .with_context(|| format!("File not found in mock filesystem: {:?}", path))?;
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.read().unwrap().contains_key(path)
            || self.directories.read().unwrap().contains_key(path)
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.directories.read().unwrap().contains_key(path)
    }

    fn is_file(&self, path: &Path) -> bool {
        self.files.read().unwrap().contains_key(path)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let files = self.files.read().unwrap();
        let directories = self.directories.read().unwrap();

        let mut entries = Vec::new();

        // Add direct child files
        for file_path in files.keys() {
            if file_path.parent() == Some(path) {
                entries.push(file_path.clone());
            }
        }

        // Add direct child directories
        for dir_path in directories.keys() {
            if dir_path.parent() == Some(path) {
                entries.push(dir_path.clone());
            }
        }

        Ok(entries)
    }

    fn walk_dir(&self, path: &Path, max_depth: usize) -> Result<Vec<PathBuf>> {
        let files = self.files.read().unwrap();
        let directories = self.directories.read().unwrap();

        let mut entries = Vec::new();

        // Add the root path itself if it exists (depth 0)
        if self.is_dir(path) {
            entries.push(path.to_path_buf());
        }

        // Walk files - depth is calculated from the root
        // max_depth=0: only root
        // max_depth=1: root + immediate children
        // max_depth=2: root + children + grandchildren
        for file_path in files.keys() {
            if file_path.starts_with(path) && file_path != path {
                let relative = match file_path.strip_prefix(path) {
                    Ok(rel) => rel,
                    Err(_) => continue,
                };
                let depth = relative.components().count();
                if depth <= max_depth {
                    entries.push(file_path.clone());
                }
            }
        }

        // Walk directories
        for dir_path in directories.keys() {
            if dir_path.starts_with(path) && dir_path != path {
                let relative = match dir_path.strip_prefix(path) {
                    Ok(rel) => rel,
                    Err(_) => continue,
                };
                let depth = relative.components().count();
                if depth <= max_depth {
                    entries.push(dir_path.clone());
                }
            }
        }

        Ok(entries)
    }
}
