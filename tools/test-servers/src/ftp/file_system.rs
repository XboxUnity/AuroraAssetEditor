use std::collections::BTreeMap;

const ROOT_KEY: &str = "/";

/// One entry of a folder, as a listing reports it.
pub(crate) struct FolderEntry {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) is_folder: bool,
    pub(crate) size: u64,
}

enum NodeKind {
    Folder,
    File(Vec<u8>),
}

struct Node {
    name: String,
    kind: NodeKind,
}

/// An in-memory file system whose paths ignore case, like the console's FATX volume. A node keeps
/// the case it was created with.
#[derive(Default)]
pub(crate) struct FakeFileSystem {
    nodes: BTreeMap<String, Node>,
}

/// Splits `path` into segments, resolving it against `current` when it is relative.
pub(crate) fn resolve_path(current: &[String], path: &str) -> Vec<String> {
    let mut segments = if path.starts_with('/') {
        Vec::new()
    } else {
        current.to_vec()
    };
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            name => segments.push(name.to_owned()),
        }
    }
    segments
}

/// The case-folded identity of a path, used as the lookup key and for fault injection.
pub(crate) fn path_key(segments: &[String]) -> String {
    if segments.is_empty() {
        return ROOT_KEY.to_owned();
    }
    segments
        .iter()
        .map(|segment| format!("/{}", segment.to_lowercase()))
        .collect()
}

fn parent_key(key: &str) -> &str {
    match key.rsplit_once('/') {
        Some(("", _)) | None => ROOT_KEY,
        Some((parent, _)) => parent,
    }
}

impl FakeFileSystem {
    /// Creates the folder and any missing parents.
    pub(crate) fn add_folder(&mut self, segments: &[String]) {
        for depth in 1..=segments.len() {
            let prefix = &segments[..depth];
            self.nodes.entry(path_key(prefix)).or_insert_with(|| Node {
                name: prefix[depth - 1].clone(),
                kind: NodeKind::Folder,
            });
        }
    }

    /// Creates or replaces a file (creating missing parents); an existing file keeps its name.
    pub(crate) fn put_file(&mut self, segments: &[String], bytes: &[u8]) {
        let Some((name, parents)) = segments.split_last() else {
            return;
        };
        self.add_folder(parents);
        let node = self
            .nodes
            .entry(path_key(segments))
            .or_insert_with(|| Node {
                name: name.clone(),
                kind: NodeKind::File(Vec::new()),
            });
        node.kind = NodeKind::File(bytes.to_vec());
    }

    pub(crate) fn file(&self, segments: &[String]) -> Option<&[u8]> {
        match &self.nodes.get(&path_key(segments))?.kind {
            NodeKind::File(bytes) => Some(bytes),
            NodeKind::Folder => None,
        }
    }

    pub(crate) fn is_folder(&self, segments: &[String]) -> bool {
        segments.is_empty()
            || matches!(
                self.nodes.get(&path_key(segments)).map(|node| &node.kind),
                Some(NodeKind::Folder)
            )
    }

    /// The path spelled with the stored case of every segment.
    pub(crate) fn display_path(&self, segments: &[String]) -> String {
        let mut path = String::new();
        for depth in 1..=segments.len() {
            let name = self
                .nodes
                .get(&path_key(&segments[..depth]))
                .map_or(&segments[depth - 1], |node| &node.name);
            path.push('/');
            path.push_str(name);
        }
        path
    }

    pub(crate) fn entries_in(&self, folder: &[String]) -> Vec<FolderEntry> {
        let folder_key = path_key(folder);
        self.nodes
            .iter()
            .filter(|(key, _)| parent_key(key) == folder_key)
            .map(|(key, node)| FolderEntry {
                key: key.clone(),
                name: node.name.clone(),
                is_folder: matches!(node.kind, NodeKind::Folder),
                size: match &node.kind {
                    NodeKind::File(bytes) => bytes.len() as u64,
                    NodeKind::Folder => 0,
                },
            })
            .collect()
    }
}
