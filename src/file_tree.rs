use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct FileTreeEntry {
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct FileTree {
    root: PathBuf,
    entries: Vec<FileTreeEntry>,
    expanded: HashSet<PathBuf>,
}

impl FileTree {
    /// Read only the root's immediate children (depth 1).
    pub fn read(root: PathBuf) -> anyhow::Result<Self> {
        let mut entries = Vec::new();
        let root_entry = make_entry(&root, 0)?;
        entries.push(root_entry);
        read_children(&root, 1, &mut entries)?;
        Ok(Self {
            root,
            entries,
            expanded: HashSet::new(),
        })
    }

    pub fn empty(root: PathBuf) -> Self {
        Self {
            root,
            entries: Vec::new(),
            expanded: HashSet::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn entries(&self) -> &[FileTreeEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&FileTreeEntry> {
        self.entries.get(index)
    }

    pub fn find_path(&self, path: &Path) -> Option<usize> {
        self.entries.iter().position(|entry| entry.path == path)
    }

    pub fn first_file_index(&self) -> Option<usize> {
        self.entries.iter().position(|entry| !entry.is_dir)
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    /// Toggle expand/collapse of a directory. Returns true if the directory was expanded.
    pub fn toggle_expand(&mut self, path: &Path) -> bool {
        if self.expanded.contains(path) {
            // Collapse: remove from expanded set and remove children from entries
            self.expanded.remove(path);
            self.remove_children_of(path);
            false
        } else {
            // Expand: read one level of children and insert after the directory entry
            self.expanded.insert(path.to_path_buf());
            if let Some(pos) = self.entries.iter().position(|e| e.path == path) {
                let depth = self.entries[pos].depth;
                let mut children = Vec::new();
                let _ = read_children(path, depth + 1, &mut children);
                // Insert children right after the directory entry
                let insert_pos = pos + 1;
                // Splice in the new children
                self.entries.splice(insert_pos..insert_pos, children);
            }
            true
        }
    }

    /// Remove all descendant entries of a directory (recursively collapses nested expanded dirs).
    fn remove_children_of(&mut self, dir_path: &Path) {
        // Find position of the directory
        let Some(pos) = self.entries.iter().position(|e| e.path == dir_path) else {
            return;
        };
        let parent_depth = self.entries[pos].depth;

        // Remove all entries after `pos` that have depth > parent_depth,
        // stopping at the first entry with depth <= parent_depth
        let mut remove_end = pos + 1;
        while remove_end < self.entries.len() && self.entries[remove_end].depth > parent_depth {
            // Also remove from expanded set if it was a nested expanded dir
            if self.entries[remove_end].is_dir {
                self.expanded.remove(&self.entries[remove_end].path);
            }
            remove_end += 1;
        }
        self.entries.drain((pos + 1)..remove_end);
    }

    /// Rebuild entries from scratch, preserving the current expanded set.
    pub fn refresh(&mut self) {
        let mut entries = Vec::new();
        if let Ok(root_entry) = make_entry(&self.root, 0) {
            entries.push(root_entry);
            let _ = read_children(&self.root, 1, &mut entries);
        }

        // Re-expand previously expanded directories (depth-first)
        let expanded_snapshot: HashSet<PathBuf> = self.expanded.clone();
        // Clean expanded set — we'll re-add only dirs that still exist
        self.expanded.clear();

        let mut i = 0;
        while i < entries.len() {
            if entries[i].is_dir && expanded_snapshot.contains(&entries[i].path) {
                let depth = entries[i].depth;
                let path = entries[i].path.clone();
                self.expanded.insert(path.clone());
                let mut children = Vec::new();
                let _ = read_children(&path, depth + 1, &mut children);
                let insert_pos = i + 1;
                entries.splice(insert_pos..insert_pos, children);
            }
            i += 1;
        }

        self.entries = entries;
    }
}

/// Create a single FileTreeEntry for a path at a given depth.
fn make_entry(path: &Path, depth: usize) -> anyhow::Result<FileTreeEntry> {
    let metadata = fs::symlink_metadata(path)?;
    let is_dir = metadata.file_type().is_dir();
    let name = if depth == 0 {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| path.display().to_string())
    } else {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string())
    };

    Ok(FileTreeEntry {
        path: path.to_path_buf(),
        depth,
        is_dir,
        name,
    })
}

/// Read the immediate children of `dir` and append them to `entries`.
/// Children are sorted: directories first, then alphabetically.
/// `child_depth` is the depth to assign to each child entry.
fn read_children(
    dir: &Path,
    child_depth: usize,
    entries: &mut Vec<FileTreeEntry>,
) -> anyhow::Result<()> {

    let mut children: Vec<_> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();

    children.sort_by(|left, right| {
        let left_is_dir = fs::symlink_metadata(left)
            .map(|meta| meta.file_type().is_dir())
            .unwrap_or(false);
        let right_is_dir = fs::symlink_metadata(right)
            .map(|meta| meta.file_type().is_dir())
            .unwrap_or(false);

        right_is_dir.cmp(&left_is_dir).then_with(|| {
            left.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                .cmp(
                    &right
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase(),
                )
        })
    });

    for child in children {
        if let Ok(entry) = make_entry(&child, child_depth) {
            entries.push(entry);
        }
    }

    Ok(())
}
