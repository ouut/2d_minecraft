use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Watcher for tracking file modification timestamps (`mtime`) of `.scm` Scheme script files
/// within a directory and its subdirectories recursively.
#[derive(Debug, Clone)]
pub struct ScriptWatcher {
    dir: PathBuf,
    mtimes: HashMap<PathBuf, SystemTime>,
}

impl ScriptWatcher {
    /// Create a new watcher for the specified directory and take an initial snapshot
    /// of all `.scm` script files.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let mut watcher = Self {
            dir,
            mtimes: HashMap::new(),
        };
        watcher.scan_dir();
        watcher
    }

    /// Access the directory path being watched.
    pub fn watched_dir(&self) -> &Path {
        &self.dir
    }

    /// Number of tracked `.scm` files currently recorded.
    pub fn file_count(&self) -> usize {
        self.mtimes.len()
    }

    /// Scan directory and update recorded timestamps.
    fn scan_dir(&mut self) {
        self.mtimes.clear();
        let mut files = HashMap::new();
        Self::collect_scm_files(&self.dir, &mut files);
        self.mtimes = files;
    }

    /// Recursively collect all `.scm` files and their modification times.
    fn collect_scm_files(dir: &Path, files: &mut HashMap<PathBuf, SystemTime>) {
        if !dir.is_dir() {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    Self::collect_scm_files(&path, files);
                } else if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "scm" {
                            if let Ok(metadata) = entry.metadata() {
                                if let Ok(mtime) = metadata.modified() {
                                    files.insert(path, mtime);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Checks if any `.scm` files in the watched directory (or subdirectories)
    /// have been modified, created, or deleted since the last check.
    ///
    /// Updates the internal snapshot and returns `true` if changes were detected, `false` otherwise.
    pub fn check_modified(&mut self) -> bool {
        let mut new_files = HashMap::new();
        Self::collect_scm_files(&self.dir, &mut new_files);

        let modified = new_files != self.mtimes;
        if modified {
            self.mtimes = new_files;
        }
        modified
    }

    /// Get sorted list of all currently tracked `.scm` files.
    pub fn tracked_files(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.mtimes.keys().cloned().collect();
        paths.sort();
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn test_script_watcher() {
        let temp_dir = std::env::temp_dir().join("test_script_watcher_dir");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let sub_dir = temp_dir.join("sub");
        fs::create_dir_all(&sub_dir).unwrap();

        let file1 = temp_dir.join("script1.scm");
        let file2 = sub_dir.join("script2.scm");
        let non_scm = temp_dir.join("ignore.txt");

        {
            let mut f1 = File::create(&file1).unwrap();
            writeln!(f1, "(display 1)").unwrap();
            let mut f2 = File::create(&file2).unwrap();
            writeln!(f2, "(display 2)").unwrap();
            let mut f3 = File::create(&non_scm).unwrap();
            writeln!(f3, "ignored").unwrap();
        }

        let mut watcher = ScriptWatcher::new(&temp_dir);
        assert_eq!(watcher.file_count(), 2);
        assert!(!watcher.check_modified()); // No change yet

        // Modify file1
        std::thread::sleep(std::time::Duration::from_millis(50));
        {
            let mut f1 = File::create(&file1).unwrap();
            writeln!(f1, "(display 100)").unwrap();
        }

        assert!(watcher.check_modified());
        assert!(!watcher.check_modified()); // Already updated snapshot

        // Add a new .scm file
        let file3 = sub_dir.join("script3.scm");
        {
            let mut f3 = File::create(&file3).unwrap();
            writeln!(f3, "(display 3)").unwrap();
        }
        assert!(watcher.check_modified());
        assert_eq!(watcher.file_count(), 3);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
