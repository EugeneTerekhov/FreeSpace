//! Versioned `.fssnap` snapshot container (FR-038…FR-041).
//!
//! The container is `FSSNAP\0\0`, a little-endian `u32` container version and a
//! zstd frame holding a CBOR [`SnapshotPayload`]. The payload stores the tree in
//! columnar form so millions of nodes stay compact and future readers can stream
//! the columns.
//!
//! The checksum covers the canonical CBOR encoding of the payload with the
//! `checksum` field cleared; that avoids the circular dependency of hashing a
//! document that contains its own digest.

use std::{
    fs,
    path::{Path, PathBuf},
};

use ciborium::de::from_reader;
use ciborium::ser::into_writer;
use freespace_core::{
    Error, Result, Timestamp, Uuid,
    model::Volume,
    size::Units,
    tree::{FsTree, NodeId, NodeKind},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Magic prefix of the container.
pub const MAGIC: [u8; 8] = *b"FSSNAP\0\0";
/// Current container version.
pub const CONTAINER_VERSION: u32 = 1;
/// Current payload schema version.
pub const FORMAT_VERSION: u32 = 1;
/// Parent index value that marks a root node.
pub const NO_PARENT: u32 = u32::MAX;

/// Volume metadata stored with a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotVolume {
    /// Stable domain identifier.
    pub id: String,
    /// Drive root or mount point.
    pub mount_point: String,
    /// Volume label.
    pub label: String,
    /// File-system name.
    pub filesystem: String,
    /// Total size in bytes.
    pub total_bytes: u64,
    /// Free space at creation time.
    pub free_bytes: u64,
    /// Cluster size, or zero when unknown.
    pub cluster_size: u32,
    /// Scanner that produced the tree.
    pub scan_method: String,
    /// Whether the scan ran elevated.
    pub elevated: bool,
}

/// Analysis settings captured with a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSettings {
    /// Unit system name.
    pub units: String,
    /// Whether links were followed.
    pub follow_links: bool,
    /// Whether alternate data streams were counted.
    pub include_ads: bool,
}

impl Default for SnapshotSettings {
    fn default() -> Self {
        Self {
            units: Units::Binary.as_str().to_owned(),
            follow_links: false,
            include_ads: false,
        }
    }
}

/// Columnar representation of the analysed tree.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnarNodes {
    /// Parent index per node; [`NO_PARENT`] for roots.
    pub parent: Vec<u32>,
    /// Offset of the name in `name_blob`.
    pub name_offset: Vec<u32>,
    /// Name length in UTF-8 bytes.
    pub name_length: Vec<u16>,
    /// Concatenated UTF-8 names.
    pub name_blob: Vec<u8>,
    /// 0=file, 1=directory, 2=symlink, 3=junction, 4=hard link.
    pub kind: Vec<u8>,
    /// Logical size.
    pub logical_size: Vec<u64>,
    /// Allocated size.
    pub on_disk_size: Vec<u64>,
    /// Physical size with hard links counted once.
    pub physical_size: Vec<u64>,
    /// Modification time in Unix seconds.
    pub mtime: Vec<i64>,
    /// Raw Windows attribute mask.
    pub attributes: Vec<u32>,
    /// NTFS file reference number, zero when unknown.
    pub file_ref: Vec<u64>,
    /// 0=ok, 1=denied, 2=error.
    pub access: Vec<u8>,
    /// Owner index in `owners`; [`NO_PARENT`] when unknown.
    pub owner_id: Vec<u32>,
    /// Owner identifiers.
    pub owners: Vec<String>,
}

impl ColumnarNodes {
    /// Number of stored nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Returns `true` when no node is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Returns the name of a node.
    #[must_use]
    pub fn name(&self, index: usize) -> &str {
        let offset = self.name_offset[index] as usize;
        let length = usize::from(self.name_length[index]);

        std::str::from_utf8(&self.name_blob[offset..offset + length]).unwrap_or_default()
    }

    /// Returns the relative path (from the snapshot root) of a node.
    ///
    /// Roots yield an empty path; every child is its parent joined with `\`.
    #[must_use]
    pub fn relative_path(&self, index: usize) -> String {
        let mut components = Vec::new();
        let mut current = Some(index);

        while let Some(node) = current {
            let parent = self.parent[node];
            if parent == NO_PARENT {
                break;
            }
            components.push(self.name(node));
            current = Some(parent as usize);
        }

        components.reverse();
        components.join("\\")
    }
}

/// CBOR document stored inside a `.fssnap` container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotPayload {
    /// Payload schema version.
    pub format_version: u32,
    /// Application version that wrote the file.
    pub app_version: String,
    /// Creation instant in ISO 8601 UTC.
    pub created_at: String,
    /// SHA-256 of the canonical CBOR with this field cleared.
    pub checksum: String,
    /// Volume metadata.
    pub volume: SnapshotVolume,
    /// Analysed roots.
    pub roots: Vec<String>,
    /// Number of stored nodes.
    pub node_count: u64,
    /// Analysis settings.
    pub settings: SnapshotSettings,
    /// Columnar tree.
    pub nodes: ColumnarNodes,
}

/// Builds a snapshot payload from a finished scan.
///
/// # Errors
/// Returns an error when a name is longer than 65 535 bytes or the tree cannot
/// be represented.
pub fn from_tree(
    tree: &FsTree,
    volume: &Volume,
    roots: &[PathBuf],
    settings: SnapshotSettings,
    created_at: Timestamp,
) -> Result<SnapshotPayload> {
    let mut index_of = vec![None; tree.len()];
    let mut live = Vec::new();
    for id in 0..u32::try_from(tree.len()).unwrap_or(u32::MAX) {
        if tree.node(id).is_removed() {
            continue;
        }
        let index = u32::try_from(live.len())
            .map_err(|_| Error::Other("snapshot exceeds the node limit".to_owned()))?;
        index_of[id as usize] = Some(index);
        live.push(id);
    }

    let mut nodes = ColumnarNodes {
        parent: Vec::with_capacity(live.len()),
        name_offset: Vec::with_capacity(live.len()),
        name_length: Vec::with_capacity(live.len()),
        name_blob: Vec::new(),
        kind: Vec::with_capacity(live.len()),
        logical_size: Vec::with_capacity(live.len()),
        on_disk_size: Vec::with_capacity(live.len()),
        physical_size: Vec::with_capacity(live.len()),
        mtime: Vec::with_capacity(live.len()),
        attributes: Vec::with_capacity(live.len()),
        file_ref: Vec::with_capacity(live.len()),
        access: Vec::with_capacity(live.len()),
        owner_id: Vec::with_capacity(live.len()),
        owners: Vec::new(),
    };
    let mut owner_index: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();

    for id in live {
        let node = tree.node(id);
        let parent = node
            .parent()
            .and_then(|parent| index_of[parent as usize])
            .unwrap_or(NO_PARENT);
        let name = tree.name(id);
        let name_length = u16::try_from(name.len()).map_err(|_| {
            Error::Unsupported(format!(
                "snapshot names longer than {} bytes are not supported",
                u16::MAX
            ))
        })?;
        let offset = u32::try_from(nodes.name_blob.len())
            .map_err(|_| Error::Other("snapshot name blob exceeds 4 GiB".to_owned()))?;
        nodes.name_blob.extend_from_slice(name.as_bytes());
        nodes.parent.push(parent);
        nodes.name_offset.push(offset);
        nodes.name_length.push(name_length);
        nodes.kind.push(kind_code(node.kind));
        nodes.logical_size.push(node.size.logical);
        nodes.on_disk_size.push(node.size.on_disk);
        nodes.physical_size.push(node.size.physical);
        nodes
            .mtime
            .push(node.timestamps.modified.as_millis().div_euclid(1_000));
        nodes.attributes.push(node.attributes.bits());
        nodes.file_ref.push(node.file_ref);
        nodes.access.push(access_code(node.access));
        let owner = match node.owner_id {
            Some(owner) => match owner_index.get(&owner) {
                Some(index) => *index,
                None => {
                    let index = u32::try_from(nodes.owners.len()).unwrap_or(NO_PARENT);
                    nodes.owners.push(owner.to_string());
                    owner_index.insert(owner, index);
                    index
                }
            },
            None => NO_PARENT,
        };
        nodes.owner_id.push(owner);
    }

    let node_count = u64::try_from(nodes.len()).unwrap_or(u64::MAX);
    let roots = if roots.is_empty() {
        tree.roots().map(|id| tree.path(id)).collect()
    } else {
        roots.to_vec()
    };

    let mut payload = SnapshotPayload {
        format_version: FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        created_at: created_at.format_iso8601(),
        checksum: String::new(),
        volume: SnapshotVolume {
            id: volume.id.to_string(),
            mount_point: volume.mount_point.to_string_lossy().into_owned(),
            label: volume.label.clone(),
            filesystem: volume.file_system.as_str().to_owned(),
            total_bytes: volume.total_bytes,
            free_bytes: volume.free_bytes,
            cluster_size: volume.cluster_size.unwrap_or(0),
            scan_method: volume
                .scan_method
                .map_or_else(|| "walk".to_owned(), |method| method.as_str().to_owned()),
            elevated: volume.is_elevated,
        },
        roots: roots
            .iter()
            .map(|root| root.to_string_lossy().into_owned())
            .collect(),
        node_count,
        settings,
        nodes,
    };
    payload.checksum = payload.compute_checksum()?;

    Ok(payload)
}

impl SnapshotPayload {
    /// Recomputes and verifies the checksum.
    ///
    /// # Errors
    /// Returns [`Error::CorruptedSnapshot`] when the digest does not match.
    pub fn verify(&self) -> Result<()> {
        let expected = self.compute_checksum()?;
        if expected != self.checksum {
            return Err(Error::CorruptedSnapshot(format!(
                "checksum mismatch: stored `{}`, computed `{expected}`",
                self.checksum
            )));
        }

        Ok(())
    }

    /// Computes the checksum of the canonical payload encoding.
    ///
    /// # Errors
    /// Returns an error when the payload cannot be serialised.
    pub fn compute_checksum(&self) -> Result<String> {
        let mut canonical = self.clone();
        canonical.checksum.clear();
        let mut bytes = Vec::new();
        into_writer(&canonical, &mut bytes)
            .map_err(|error| Error::Other(format!("cannot encode a snapshot: {error}")))?;

        Ok(hex(&Sha256::digest(&bytes)))
    }

    /// Writes the snapshot to `path`.
    ///
    /// # Errors
    /// Returns an error when the payload cannot be encoded or the file cannot be
    /// written.
    pub fn write(&self, path: &Path) -> Result<()> {
        let mut payload = self.clone();
        payload.checksum = payload.compute_checksum()?;
        let mut cbor = Vec::new();
        into_writer(&payload, &mut cbor)
            .map_err(|error| Error::Other(format!("cannot encode a snapshot: {error}")))?;
        let compressed = zstd::encode_all(cbor.as_slice(), 3)?;

        let mut container = Vec::with_capacity(MAGIC.len() + 4 + compressed.len());
        container.extend_from_slice(&MAGIC);
        container.extend_from_slice(&CONTAINER_VERSION.to_le_bytes());
        container.extend_from_slice(&compressed);

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(path, container)?;

        Ok(())
    }

    /// Reads and verifies a snapshot.
    ///
    /// # Errors
    /// Returns [`Error::CorruptedSnapshot`] for a damaged container or payload
    /// and [`Error::IncompatibleSnapshot`] for a newer schema version.
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        if bytes.len() < MAGIC.len() + 4 || bytes[..MAGIC.len()] != MAGIC {
            return Err(Error::CorruptedSnapshot(format!(
                "`{}` is not a FreeSpace snapshot",
                path.display()
            )));
        }
        let version = u32::from_le_bytes(
            bytes[MAGIC.len()..MAGIC.len() + 4]
                .try_into()
                .map_err(|_| Error::CorruptedSnapshot("truncated container header".to_owned()))?,
        );
        if version > CONTAINER_VERSION {
            return Err(Error::IncompatibleSnapshot(format!(
                "container version {version} is newer than the supported {CONTAINER_VERSION}"
            )));
        }

        let cbor = zstd::decode_all(&bytes[MAGIC.len() + 4..])
            .map_err(|error| Error::CorruptedSnapshot(format!("cannot decompress: {error}")))?;
        let payload: Self = from_reader(cbor.as_slice())
            .map_err(|error| Error::CorruptedSnapshot(format!("cannot decode: {error}")))?;
        if payload.format_version > FORMAT_VERSION {
            return Err(Error::IncompatibleSnapshot(format!(
                "payload format {} is newer than the supported {FORMAT_VERSION}",
                payload.format_version
            )));
        }
        if payload.nodes.len() != usize::try_from(payload.node_count).unwrap_or(usize::MAX) {
            return Err(Error::CorruptedSnapshot(
                "node_count does not match the column lengths".to_owned(),
            ));
        }
        let count = payload.nodes.len();
        if payload.nodes.kind.len() != count
            || payload.nodes.logical_size.len() != count
            || payload.nodes.on_disk_size.len() != count
            || payload.nodes.physical_size.len() != count
            || payload.nodes.mtime.len() != count
            || payload.nodes.attributes.len() != count
            || payload.nodes.file_ref.len() != count
            || payload.nodes.access.len() != count
            || payload.nodes.owner_id.len() != count
        {
            return Err(Error::CorruptedSnapshot(
                "the column lengths differ".to_owned(),
            ));
        }
        for parent in &payload.nodes.parent {
            if *parent != NO_PARENT && usize::try_from(*parent).unwrap_or(usize::MAX) >= count {
                return Err(Error::CorruptedSnapshot(
                    "a parent index is out of range".to_owned(),
                ));
            }
        }
        for index in 0..count {
            let offset = usize::try_from(payload.nodes.name_offset[index]).unwrap_or(usize::MAX);
            let length = usize::from(payload.nodes.name_length[index]);
            if offset.saturating_add(length) > payload.nodes.name_blob.len() {
                return Err(Error::CorruptedSnapshot(
                    "a name reference is out of range".to_owned(),
                ));
            }
        }
        payload.verify()?;

        Ok(payload)
    }

    /// Stable identifier of the snapshot volume.
    #[must_use]
    pub fn volume_id(&self) -> Option<Uuid> {
        Uuid::parse_str(&self.volume.id).ok()
    }

    /// Relative paths of every non-root node, together with their index.
    #[must_use]
    pub fn relative_paths(&self) -> Vec<(usize, String)> {
        (0..self.nodes.len())
            .filter(|index| self.nodes.parent[*index] != NO_PARENT)
            .map(|index| (index, self.nodes.relative_path(index)))
            .collect()
    }
}

const fn kind_code(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::File => 0,
        NodeKind::Directory => 1,
        NodeKind::Symlink => 2,
        NodeKind::Junction => 3,
        NodeKind::Hardlink => 4,
    }
}

const fn access_code(access: freespace_core::tree::AccessState) -> u8 {
    use freespace_core::tree::AccessState;

    match access {
        AccessState::Ok => 0,
        AccessState::Denied => 1,
        AccessState::Error => 2,
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }

    output
}

/// Returns `true` when the index refers to a directory.
#[must_use]
pub fn is_directory(nodes: &ColumnarNodes, index: usize) -> bool {
    nodes.kind.get(index) == Some(&1)
}

/// Returns the node identifier used in [`NO_PARENT`] sentinel checks.
#[must_use]
pub const fn no_parent() -> NodeId {
    NodeId::MAX
}
