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
}

impl FileTree {
    pub fn read(root: PathBuf) -> anyhow::Result<Self> {
        let mut entries = Vec::new();
        build_entries(&root, 0, &mut entries)?;
        Ok(Self { root, entries })
    }

    pub fn empty(root: PathBuf) -> Self {
        Self {
            root,
            entries: Vec::new(),
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
}

fn build_entries(
    path: &Path,
    depth: usize,
    entries: &mut Vec<FileTreeEntry>,
) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();
    let is_dir = file_type.is_dir();
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

    entries.push(FileTreeEntry {
        path: path.to_path_buf(),
        depth,
        is_dir,
        name,
    });

    if !is_dir || file_type.is_symlink() {
        return Ok(());
    }

    let mut children: Vec<_> = fs::read_dir(path)?
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
        build_entries(&child, depth + 1, entries)?;
    }

    Ok(())
}
