//! Arena-backed file-system tree.
//!
//! Millions of nodes must fit into a few gigabytes, so the tree is stored as
//! two flat arrays instead of linked heap objects:
//!
//! * `FsNode` records hold parent/child/sibling indices, size metrics and
//!   metadata, but no inline strings;
//! * every name is interned into one UTF-8 buffer and referenced by offset and
//!   length.
//!
//! Children keep insertion order through a first-child / next-sibling chain and
//! a cached tail index, so appending a child is O(1).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{Error, Result, size::SizeMetrics, time::Timestamp};

/// Index of a node inside [`FsTree`].
pub type NodeId = u32;

/// What a node represents.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    /// Regular file.
    #[default]
    File,
    /// Directory.
    Directory,
    /// Symbolic link.
    Symlink,
    /// NTFS junction point.
    Junction,
    /// Additional hard link to an existing file.
    Hardlink,
}

impl NodeKind {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::Symlink => "symlink",
            Self::Junction => "junction",
            Self::Hardlink => "hardlink",
        }
    }

    /// Returns `true` for directories.
    #[must_use]
    pub const fn is_directory(self) -> bool {
        matches!(self, Self::Directory)
    }

    /// Returns `true` for symbolic links, junctions and hard links.
    #[must_use]
    pub const fn is_link(self) -> bool {
        matches!(self, Self::Symlink | Self::Junction | Self::Hardlink)
    }
}

/// Whether the scanner could read an object.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessState {
    /// Object was read successfully.
    #[default]
    Ok,
    /// Access was denied; sizes are incomplete.
    Denied,
    /// Reading failed for another reason.
    Error,
}

impl AccessState {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Denied => "denied",
            Self::Error => "error",
        }
    }
}

/// Bit mask of Windows file attributes relevant to the analysis.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FileAttributes(u32);

impl FileAttributes {
    /// Read-only attribute.
    pub const READ_ONLY: Self = Self(0x0000_0001);
    /// Hidden attribute.
    pub const HIDDEN: Self = Self(0x0000_0002);
    /// System attribute.
    pub const SYSTEM: Self = Self(0x0000_0004);
    /// Directory attribute.
    pub const DIRECTORY: Self = Self(0x0000_0010);
    /// Archive attribute.
    pub const ARCHIVE: Self = Self(0x0000_0020);
    /// Sparse-file attribute.
    pub const SPARSE_FILE: Self = Self(0x0000_0200);
    /// Reparse-point attribute (symlink or junction).
    pub const REPARSE_POINT: Self = Self(0x0000_0400);
    /// NTFS-compressed attribute.
    pub const COMPRESSED: Self = Self(0x0000_0800);

    /// No attributes set.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Wraps a raw `dwFileAttributes` value from WinAPI.
    #[must_use]
    pub const fn from_windows_attributes(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the raw bit mask.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns `true` when every bit of `other` is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns `true` when no attribute is set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Sets the bits of `other`.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Clears the bits of `other`.
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

/// Creation, modification and access timestamps of an object.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamps {
    /// Creation time.
    pub created: Timestamp,
    /// Last modification time.
    pub modified: Timestamp,
    /// Last access time.
    pub accessed: Timestamp,
}

/// One NTFS alternate data stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdsStream {
    /// Stream name as reported by `FindFirstStreamW`, for example `Zone.Identifier`.
    pub name: String,
    /// Stream length in bytes.
    pub size: u64,
}

/// Byte range inside the tree name arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NameRef {
    start: u32,
    len: u32,
}

/// A single file-system object inside [`FsTree`].
#[derive(Debug, Clone)]
pub struct FsNode {
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    last_child: Option<NodeId>,
    next_sibling: Option<NodeId>,
    name: NameRef,
    link_target: Option<NameRef>,
    /// Object type.
    pub kind: NodeKind,
    /// NTFS file reference number used for hard-link de-duplication.
    pub file_ref: u64,
    /// Size along every dimension.
    pub size: SizeMetrics,
    /// Object timestamps (UTC).
    pub timestamps: Timestamps,
    /// Windows file attributes.
    pub attributes: FileAttributes,
    /// Index into the owner table, when known.
    pub owner_id: Option<u32>,
    /// Whether the scanner could read the object.
    pub access: AccessState,
    /// Whether the object changed while the scan was running.
    pub changed_during_scan: bool,
    /// Alternate data streams; absent for the vast majority of nodes.
    pub ads: Option<Box<[AdsStream]>>,
}

impl FsNode {
    /// Parent node, or `None` for a scan root.
    #[must_use]
    pub const fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    /// First child node.
    #[must_use]
    pub const fn first_child(&self) -> Option<NodeId> {
        self.first_child
    }

    /// Next sibling node.
    #[must_use]
    pub const fn next_sibling(&self) -> Option<NodeId> {
        self.next_sibling
    }

    /// Returns `true` when the object is a directory.
    #[must_use]
    pub const fn is_directory(&self) -> bool {
        self.kind.is_directory()
    }

    /// Returns `true` when the scanner could read the object.
    #[must_use]
    pub const fn is_accessible(&self) -> bool {
        matches!(self.access, AccessState::Ok)
    }
}

/// Description accepted by [`FsTree::add_node`].
#[derive(Debug, Clone, Default)]
pub struct NodeSpec {
    /// Name without any parent path.
    pub name: String,
    /// Object type.
    pub kind: NodeKind,
    /// NTFS file reference number.
    pub file_ref: u64,
    /// Size metrics.
    pub size: SizeMetrics,
    /// Timestamps in UTC.
    pub timestamps: Timestamps,
    /// Windows file attributes.
    pub attributes: FileAttributes,
    /// Owner table index.
    pub owner_id: Option<u32>,
    /// Read state.
    pub access: AccessState,
    /// Whether the object changed during the scan.
    pub changed_during_scan: bool,
    /// Reparse-point target.
    pub link_target: Option<String>,
    /// Alternate data streams.
    pub ads: Vec<AdsStream>,
}

/// Flat arena holding every node of one analysis session.
#[derive(Debug, Default, Clone)]
pub struct FsTree {
    nodes: Vec<FsNode>,
    names: String,
    roots: Vec<NodeId>,
}

impl FsTree {
    /// Creates an empty tree.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty tree with pre-allocated node and name capacity.
    #[must_use]
    pub fn with_capacity(nodes: usize, name_bytes: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(nodes),
            names: String::with_capacity(name_bytes),
            roots: Vec::new(),
        }
    }

    /// Number of nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` when the tree has no nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Number of bytes currently used by the name arena.
    #[must_use]
    pub fn name_bytes(&self) -> usize {
        self.names.len()
    }

    /// Appends a node and links it as the last child of `parent`.
    ///
    /// # Errors
    /// Returns an error when `parent` does not exist or the tree grows past
    /// [`NodeId`] limits.
    pub fn add_node(&mut self, parent: Option<NodeId>, spec: NodeSpec) -> Result<NodeId> {
        if let Some(parent) = parent {
            let index = usize::try_from(parent).ok();
            if index.is_none_or(|index| index >= self.nodes.len()) {
                return Err(Error::Other(format!("parent node {parent} does not exist")));
            }
        }

        let id = u32::try_from(self.nodes.len())
            .map_err(|_| Error::Other("file-system tree exceeds the node limit".to_owned()))?;
        let name = self.intern(&spec.name)?;
        let link_target = spec
            .link_target
            .as_deref()
            .map(|target| self.intern(target))
            .transpose()?;

        let node = FsNode {
            parent,
            first_child: None,
            last_child: None,
            next_sibling: None,
            name,
            link_target,
            kind: spec.kind,
            file_ref: spec.file_ref,
            size: spec.size,
            timestamps: spec.timestamps,
            attributes: spec.attributes,
            owner_id: spec.owner_id,
            access: spec.access,
            changed_during_scan: spec.changed_during_scan,
            ads: if spec.ads.is_empty() {
                None
            } else {
                Some(spec.ads.into_boxed_slice())
            },
        };

        self.nodes.push(node);

        match parent {
            None => self.roots.push(id),
            Some(parent) => {
                let last_child = self.nodes[parent as usize].last_child;
                if let Some(last_child) = last_child {
                    self.nodes[last_child as usize].next_sibling = Some(id);
                } else {
                    self.nodes[parent as usize].first_child = Some(id);
                }
                self.nodes[parent as usize].last_child = Some(id);
            }
        }

        Ok(id)
    }

    /// Returns the node record.
    ///
    /// # Panics
    /// Panics when `id` does not belong to this tree. Scanner code always uses
    /// ids returned by [`Self::add_node`].
    #[must_use]
    pub fn node(&self, id: NodeId) -> &FsNode {
        &self.nodes[id as usize]
    }

    /// Returns the mutable node record.
    ///
    /// # Panics
    /// Panics when `id` does not belong to this tree.
    #[must_use]
    pub fn node_mut(&mut self, id: NodeId) -> &mut FsNode {
        &mut self.nodes[id as usize]
    }

    /// Returns the name of a node.
    ///
    /// # Panics
    /// Panics when `id` does not belong to this tree or the arena is corrupt.
    #[must_use]
    pub fn name(&self, id: NodeId) -> &str {
        let reference = self.nodes[id as usize].name;

        &self.names[reference.start as usize..(reference.start + reference.len) as usize]
    }

    /// Returns the reparse-point target, when known.
    ///
    /// # Panics
    /// Panics when `id` does not belong to this tree or the arena is corrupt.
    #[must_use]
    pub fn link_target(&self, id: NodeId) -> Option<&str> {
        let reference = self.nodes[id as usize].link_target?;

        Some(&self.names[reference.start as usize..(reference.start + reference.len) as usize])
    }

    /// Iterates over the children of a node in insertion order.
    #[must_use]
    pub fn children(&self, id: NodeId) -> ChildIter<'_> {
        ChildIter {
            tree: self,
            next: self.nodes[id as usize].first_child,
        }
    }

    /// Iterates over every scan root in insertion order.
    pub fn roots(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.roots.iter().copied()
    }

    /// Number of scan roots.
    #[must_use]
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }

    /// Builds the full path by walking parent links.
    ///
    /// # Panics
    /// Panics when `id` does not belong to this tree.
    #[must_use]
    pub fn path(&self, id: NodeId) -> PathBuf {
        let mut components = Vec::new();
        let mut current = Some(id);

        while let Some(node) = current {
            components.push(self.name(node));
            current = self.node(node).parent;
        }

        components.reverse();

        let mut path = PathBuf::new();
        for component in components {
            path.push(component);
        }

        path
    }

    /// Depth of a node; scan roots have depth zero.
    ///
    /// # Panics
    /// Panics when `id` does not belong to this tree.
    #[must_use]
    pub fn depth(&self, id: NodeId) -> usize {
        let mut depth = 0;
        let mut current = self.node(id).parent;

        while let Some(node) = current {
            depth += 1;
            current = self.node(node).parent;
        }

        depth
    }

    /// Checks the structural invariants of the arena.
    ///
    /// # Errors
    /// Returns an error describing the first inconsistency found.
    pub fn validate(&self) -> Result<()> {
        if self.names.len() > u32::MAX as usize {
            return Err(Error::Other("name arena exceeds 4 GiB".to_owned()));
        }

        for (index, node) in self.nodes.iter().enumerate() {
            let id = NodeId::try_from(index)
                .map_err(|_| Error::Other("node index does not fit NodeId".to_owned()))?;

            if let Some(parent) = node.parent {
                if parent as usize >= self.nodes.len() {
                    return Err(Error::Other(format!(
                        "node {id} has out-of-range parent {parent}"
                    )));
                }
                if usize::try_from(parent).ok() == Some(index) {
                    return Err(Error::Other(format!("node {id} is its own parent")));
                }
            }

            let name = self.name(id);
            if name.is_empty() {
                return Err(Error::Other(format!("node {id} has an empty name")));
            }

            if let Some(reference) = node.link_target {
                let end = u64::from(reference.start) + u64::from(reference.len);
                if end > self.names.len() as u64 {
                    return Err(Error::Other(format!(
                        "node {id} has an out-of-range link target"
                    )));
                }
            }
        }

        for root in &self.roots {
            if *root as usize >= self.nodes.len() {
                return Err(Error::Other(format!("out-of-range root {root}")));
            }
        }

        Ok(())
    }

    fn intern(&mut self, value: &str) -> Result<NameRef> {
        let start = u32::try_from(self.names.len())
            .map_err(|_| Error::Other("name arena exceeds 4 GiB".to_owned()))?;
        let len = u32::try_from(value.len())
            .map_err(|_| Error::Other("a name is longer than 4 GiB".to_owned()))?;

        self.names.push_str(value);

        Ok(NameRef { start, len })
    }
}

/// Iterator over the children of a node.
#[derive(Debug)]
pub struct ChildIter<'tree> {
    tree: &'tree FsTree,
    next: Option<NodeId>,
}

impl Iterator for ChildIter<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next?;
        self.next = self.tree.node(current).next_sibling;

        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        AccessState, AdsStream, FileAttributes, FsNode, FsTree, NodeKind, NodeSpec, SizeMetrics,
    };
    use crate::time::Timestamp;

    fn file(name: &str, bytes: u64) -> NodeSpec {
        NodeSpec {
            name: name.to_owned(),
            kind: NodeKind::File,
            size: SizeMetrics::file(bytes),
            ..NodeSpec::default()
        }
    }

    fn directory(name: &str) -> NodeSpec {
        NodeSpec {
            name: name.to_owned(),
            kind: NodeKind::Directory,
            ..NodeSpec::default()
        }
    }

    #[test]
    fn links_children_in_insertion_order() {
        let mut tree = FsTree::new();
        let root = tree.add_node(None, directory("project")).expect("root");
        let a = tree.add_node(Some(root), file("a.txt", 10)).expect("a");
        let b = tree.add_node(Some(root), file("b.txt", 20)).expect("b");
        let c = tree.add_node(Some(root), file("c.txt", 30)).expect("c");

        assert_eq!(tree.children(root).collect::<Vec<_>>(), vec![a, b, c]);
        assert_eq!(tree.node(b).parent(), Some(root));
        assert_eq!(tree.node(a).next_sibling(), Some(b));
        assert_eq!(tree.node(c).next_sibling(), None);
    }

    #[test]
    fn builds_paths_and_depths() {
        let mut tree = FsTree::new();
        let root = tree.add_node(None, directory(r"C:\")).expect("root");
        let users = tree
            .add_node(Some(root), directory("Users"))
            .expect("users");
        let alice = tree
            .add_node(Some(users), directory("Alice"))
            .expect("alice");
        let note = tree
            .add_node(Some(alice), file("note.txt", 5))
            .expect("note");

        assert_eq!(tree.path(note), PathBuf::from(r"C:\Users\Alice\note.txt"));
        assert_eq!(tree.depth(root), 0);
        assert_eq!(tree.depth(note), 3);
        assert_eq!(tree.root_count(), 1);
        assert_eq!(tree.roots().collect::<Vec<_>>(), vec![root]);
    }

    #[test]
    fn interns_unicode_names() {
        let mut tree = FsTree::new();
        let root = tree.add_node(None, directory("диск")).expect("root");
        let file = tree
            .add_node(Some(root), file("отчёт 2026.txt", 42))
            .expect("file");

        assert_eq!(tree.name(file), "отчёт 2026.txt");
        assert_eq!(
            tree.path(file),
            PathBuf::from("диск").join("отчёт 2026.txt")
        );
        assert!(tree.name_bytes() >= "диск".len() + "отчёт 2026.txt".len());
        tree.validate().expect("tree must be consistent");
    }

    #[test]
    fn stores_links_attributes_and_streams() {
        let mut tree = FsTree::new();
        let mut attributes = FileAttributes::empty();
        attributes.insert(FileAttributes::REPARSE_POINT);
        attributes.insert(FileAttributes::HIDDEN);

        let spec = NodeSpec {
            name: "link".to_owned(),
            kind: NodeKind::Junction,
            link_target: Some(r"D:\target".to_owned()),
            attributes,
            access: AccessState::Denied,
            timestamps: super::Timestamps {
                modified: Timestamp::from_millis(1_700_000_000_000),
                ..super::Timestamps::default()
            },
            ads: vec![
                AdsStream {
                    name: "Zone.Identifier".to_owned(),
                    size: 26,
                },
                AdsStream {
                    name: "custom".to_owned(),
                    size: 4,
                },
            ],
            ..NodeSpec::default()
        };

        let id = tree.add_node(None, spec).expect("node");
        let node = tree.node(id);

        assert_eq!(tree.link_target(id), Some(r"D:\target"));
        assert!(node.attributes.contains(FileAttributes::REPARSE_POINT));
        assert!(node.attributes.contains(FileAttributes::HIDDEN));
        assert_eq!(node.access, AccessState::Denied);
        assert_eq!(node.ads.as_deref().map(<[AdsStream]>::len), Some(2));
        assert!(!node.is_directory());
    }

    #[test]
    fn rejects_missing_parents() {
        let mut tree = FsTree::new();

        assert!(tree.add_node(Some(42), file("orphan", 1)).is_err());
        assert!(tree.is_empty());
    }

    #[test]
    fn appends_many_children_without_losing_order() {
        let mut tree = FsTree::with_capacity(10_001, 128);
        let root = tree.add_node(None, directory("root")).expect("root");

        for index in 0..10_000 {
            tree.add_node(Some(root), file(&format!("item-{index}"), index))
                .expect("child");
        }

        let children: Vec<_> = tree.children(root).collect();

        assert_eq!(children.len(), 10_000);
        assert_eq!(tree.name(children[0]), "item-0");
        assert_eq!(tree.name(children[9_999]), "item-9999");
    }

    #[test]
    fn keeps_node_record_compact() {
        // The memory budget allows 4 GiB for 10 million nodes. A record of up
        // to 192 bytes leaves 1.92 GiB for nodes plus room for the name arena
        // (about 40 bytes per average file name).
        assert!(
            size_of::<FsNode>() <= 192,
            "FsNode grew to {} bytes",
            size_of::<FsNode>()
        );
    }
}
