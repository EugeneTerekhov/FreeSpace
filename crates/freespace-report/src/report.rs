//! Shared report model and section schemas (FR-070…FR-073).
//!
//! Every writer receives the same [`Report`] and the same row iterators, so CSV,
//! XML, HTML and XLSX can never disagree about headers, order or totals. Row
//! iterators borrow the analysed tree and therefore stream millions of nodes
//! without materialising them.

use std::path::PathBuf;

use freespace_analysis::{DuplicateGroup, OwnerSummary, OwnerTable, TopFile, TypeGroup};
use freespace_core::{
    Timestamp,
    model::{ScanMethod, Volume},
    size::SizeMetrics,
    tree::{FileAttributes, FsTree, NodeId},
};
use freespace_platform::paths;

/// Sections a report can contain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionName {
    /// Hierarchical node list.
    Tree,
    /// Largest files.
    Top,
    /// Space by extension.
    Types,
    /// Duplicate groups.
    Duplicates,
    /// Space by owner.
    Owners,
}

impl SectionName {
    /// Every section in document order.
    pub const ALL: [Self; 5] = [
        Self::Tree,
        Self::Top,
        Self::Types,
        Self::Duplicates,
        Self::Owners,
    ];

    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Top => "top",
            Self::Types => "types",
            Self::Duplicates => "duplicates",
            Self::Owners => "owners",
        }
    }

    /// XML element used for one row of the section.
    #[must_use]
    pub const fn element_name(self) -> &'static str {
        match self {
            Self::Tree => "node",
            Self::Top => "file",
            Self::Types => "type",
            Self::Duplicates => "duplicate",
            Self::Owners => "owner",
        }
    }

    /// XLSX sheet name.
    #[must_use]
    pub const fn sheet_name(self) -> &'static str {
        match self {
            Self::Tree => "Tree",
            Self::Top => "TopFiles",
            Self::Types => "Types",
            Self::Duplicates => "Duplicates",
            Self::Owners => "Owners",
        }
    }

    /// Column headers in the documented order.
    #[must_use]
    pub const fn headers(self) -> &'static [&'static str] {
        match self {
            Self::Tree => &[
                "path",
                "name",
                "kind",
                "logical_size",
                "on_disk_size",
                "physical_size",
                "files",
                "dirs",
                "modified",
                "attributes",
                "owner",
            ],
            Self::Top => &["rank", "path", "size", "modified"],
            Self::Types => &["extension", "category", "count", "total_size", "share"],
            Self::Duplicates => &[
                "group_id",
                "hash_algorithm",
                "hash",
                "size",
                "path",
                "keep",
                "selected",
                "reclaimable_bytes",
            ],
            Self::Owners => &["owner", "sid", "files", "total_size"],
        }
    }

    /// Parses a section name from the `--include` option.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "tree" => Some(Self::Tree),
            "top" => Some(Self::Top),
            "types" => Some(Self::Types),
            "duplicates" | "dups" => Some(Self::Duplicates),
            "owners" => Some(Self::Owners),
            _ => None,
        }
    }
}

/// A cell value with its type preserved for XLSX and XML.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// Free text.
    Text(String),
    /// Unsigned integer.
    Unsigned(u64),
    /// Signed integer.
    Signed(i64),
    /// Floating-point number.
    Float(f64),
    /// Boolean.
    Bool(bool),
}

impl Cell {
    /// Renders the value for CSV and HTML.
    #[must_use]
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(value) => value.clone(),
            Self::Unsigned(value) => value.to_string(),
            Self::Signed(value) => value.to_string(),
            Self::Float(value) => format!("{value:.6}"),
            Self::Bool(value) => value.to_string(),
        }
    }

    /// Returns the numeric value when the cell is an integer or a float.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Unsigned(value) => Some(*value as f64),
            Self::Signed(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }
}

/// Volume metadata captured with a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportVolume {
    /// Drive letter or mount point.
    pub letter: String,
    /// Volume label.
    pub label: String,
    /// File-system name.
    pub filesystem: String,
    /// Scanning method that produced the tree.
    pub scan_method: String,
}

impl ReportVolume {
    /// Captures a scanned volume.
    #[must_use]
    pub fn from_volume(volume: &Volume) -> Self {
        let mount = volume.mount_point.to_string_lossy();
        let letter = mount
            .trim_end_matches(['\\', '/'])
            .trim_end_matches(':')
            .to_owned();
        Self {
            letter,
            label: volume.label.clone(),
            filesystem: volume.file_system.as_str().to_owned(),
            scan_method: volume
                .scan_method
                .map(ScanMethod::as_str)
                .unwrap_or("unknown")
                .to_owned(),
        }
    }
}

/// Report metadata shared by every format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportMeta {
    /// Schema version of the report.
    pub report_version: u32,
    /// Generation time in UTC.
    pub generated_at: Timestamp,
    /// Application version.
    pub app_version: String,
    /// Scanned volume, when the report covers one.
    pub volume: Option<ReportVolume>,
    /// Analysed roots.
    pub roots: Vec<String>,
    /// Scan session identifier.
    pub session_id: String,
    /// Unit system name.
    pub units: String,
}

impl Default for ReportMeta {
    fn default() -> Self {
        Self {
            report_version: Self::VERSION,
            generated_at: Timestamp::UNIX_EPOCH,
            app_version: String::new(),
            volume: None,
            roots: Vec::new(),
            session_id: String::new(),
            units: "binary".to_owned(),
        }
    }
}

impl ReportMeta {
    /// Current report schema version.
    pub const VERSION: u32 = 1;

    /// Builds metadata with the current schema version.
    #[must_use]
    pub fn new(generated_at: Timestamp, app_version: impl Into<String>) -> Self {
        Self {
            report_version: Self::VERSION,
            generated_at,
            app_version: app_version.into(),
            volume: None,
            roots: Vec::new(),
            session_id: String::new(),
            units: "binary".to_owned(),
        }
    }
    /// Adds volume information.
    #[must_use]
    pub fn with_volume(mut self, volume: ReportVolume) -> Self {
        self.volume = Some(volume);
        self
    }

    /// Adds the analysed roots.
    #[must_use]
    pub fn with_roots<I, S>(mut self, roots: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.roots = roots.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the scan session identifier.
    #[must_use]
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = session_id.into();
        self
    }

    /// Sets the unit system name.
    #[must_use]
    pub fn with_units(mut self, units: impl Into<String>) -> Self {
        self.units = units.into();
        self
    }
}

/// One row of the tree section.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRow {
    /// Full path.
    pub path: String,
    /// Name without directories.
    pub name: String,
    /// Object kind.
    pub kind: &'static str,
    /// Logical size.
    pub logical_size: u64,
    /// Allocated size.
    pub on_disk_size: u64,
    /// Physical size with hard links counted once.
    pub physical_size: u64,
    /// Number of files below the node.
    pub files: u32,
    /// Number of directories below the node.
    pub dirs: u32,
    /// Modification time.
    pub modified: Timestamp,
    /// Attributes rendered as a comma-separated list.
    pub attributes: String,
    /// Owner display name, when known.
    pub owner: String,
}

impl TreeRow {
    /// Cells in the documented order.
    #[must_use]
    pub fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::Text(self.path.clone()),
            Cell::Text(self.name.clone()),
            Cell::Text(self.kind.to_owned()),
            Cell::Unsigned(self.logical_size),
            Cell::Unsigned(self.on_disk_size),
            Cell::Unsigned(self.physical_size),
            Cell::Unsigned(u64::from(self.files)),
            Cell::Unsigned(u64::from(self.dirs)),
            Cell::Text(self.modified.format_iso8601()),
            Cell::Text(self.attributes.clone()),
            Cell::Text(self.owner.clone()),
        ]
    }
}

/// One row of the top-files section.
#[derive(Debug, Clone, PartialEq)]
pub struct TopRow {
    /// Rank, starting at one.
    pub rank: u32,
    /// Full path.
    pub path: String,
    /// Logical size.
    pub size: u64,
    /// Modification time.
    pub modified: Timestamp,
}

impl TopRow {
    /// Cells in the documented order.
    #[must_use]
    pub fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::Unsigned(u64::from(self.rank)),
            Cell::Text(self.path.clone()),
            Cell::Unsigned(self.size),
            Cell::Text(self.modified.format_iso8601()),
        ]
    }
}

/// One row of the types section.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeRow {
    /// Extension, or `<none>`.
    pub extension: String,
    /// Coarse category.
    pub category: String,
    /// Number of files.
    pub count: u64,
    /// Total logical size.
    pub total_size: u64,
    /// Share in `0.0..=1.0`.
    pub share: f64,
}

impl TypeRow {
    /// Cells in the documented order.
    #[must_use]
    pub fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::Text(self.extension.clone()),
            Cell::Text(self.category.clone()),
            Cell::Unsigned(self.count),
            Cell::Unsigned(self.total_size),
            Cell::Float(self.share),
        ]
    }
}

/// One row of the duplicates section.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateRow {
    /// Group identifier.
    pub group_id: String,
    /// Hash algorithm.
    pub hash_algorithm: String,
    /// Full digest.
    pub hash: String,
    /// File size.
    pub size: u64,
    /// Member path.
    pub path: String,
    /// Whether this member is recommended to keep.
    pub keep: bool,
    /// Whether this member is selected for deletion.
    pub selected: bool,
    /// Reclaimable bytes, non-zero on the first row of a group.
    pub reclaimable_bytes: u64,
}

impl DuplicateRow {
    /// Cells in the documented order.
    #[must_use]
    pub fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::Text(self.group_id.clone()),
            Cell::Text(self.hash_algorithm.clone()),
            Cell::Text(self.hash.clone()),
            Cell::Unsigned(self.size),
            Cell::Text(self.path.clone()),
            Cell::Bool(self.keep),
            Cell::Bool(self.selected),
            Cell::Unsigned(self.reclaimable_bytes),
        ]
    }
}

/// One row of the owners section.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnerRow {
    /// Owner display name.
    pub owner: String,
    /// Security identifier.
    pub sid: String,
    /// Number of files.
    pub files: u64,
    /// Total logical size.
    pub total_size: u64,
}

impl OwnerRow {
    /// Cells in the documented order.
    #[must_use]
    pub fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::Text(self.owner.clone()),
            Cell::Text(self.sid.clone()),
            Cell::Unsigned(self.files),
            Cell::Unsigned(self.total_size),
        ]
    }
}

/// A complete report ready for one of the writers.
#[derive(Debug)]
pub struct Report<'a> {
    /// Report metadata.
    pub meta: ReportMeta,
    /// Analysed tree.
    pub tree: Option<&'a FsTree>,
    /// Largest files.
    pub top: Option<&'a [TopFile]>,
    /// Space by extension.
    pub types: Option<&'a [TypeGroup]>,
    /// Duplicate groups.
    pub duplicates: Option<&'a [DuplicateGroup]>,
    /// Owner summaries.
    pub owners: Option<&'a [OwnerSummary]>,
    /// Owner table used to resolve tree node owners.
    pub owner_table: Option<&'a OwnerTable>,
    /// Sections to include.
    pub include: Vec<SectionName>,
    /// Cap on tree rows in every format.
    pub max_tree_rows: Option<usize>,
    /// Maximum tree depth for HTML.
    pub html_tree_depth: Option<usize>,
    /// Totals shown in the report footer.
    pub totals: SizeMetrics,
}

impl<'a> Report<'a> {
    /// Builds a report with every section enabled.
    #[must_use]
    pub fn new(meta: ReportMeta, totals: SizeMetrics) -> Self {
        Self {
            meta,
            tree: None,
            top: None,
            types: None,
            duplicates: None,
            owners: None,
            owner_table: None,
            include: SectionName::ALL.to_vec(),
            max_tree_rows: None,
            html_tree_depth: Some(8),
            totals,
        }
    }

    /// Attaches the analysed tree.
    #[must_use]
    pub fn with_tree(mut self, tree: &'a FsTree) -> Self {
        self.tree = Some(tree);
        self
    }

    /// Attaches the top-file list.
    #[must_use]
    pub fn with_top(mut self, top: &'a [TopFile]) -> Self {
        self.top = Some(top);
        self
    }

    /// Attaches the type list.
    #[must_use]
    pub fn with_types(mut self, types: &'a [TypeGroup]) -> Self {
        self.types = Some(types);
        self
    }

    /// Attaches duplicate groups.
    #[must_use]
    pub fn with_duplicates(mut self, duplicates: &'a [DuplicateGroup]) -> Self {
        self.duplicates = Some(duplicates);
        self
    }

    /// Attaches the owner report.
    #[must_use]
    pub fn with_owners(mut self, owners: &'a [OwnerSummary], table: &'a OwnerTable) -> Self {
        self.owners = Some(owners);
        self.owner_table = Some(table);
        self
    }

    /// Restricts the report to the requested sections.
    #[must_use]
    pub fn with_include(mut self, include: Vec<SectionName>) -> Self {
        self.include = include;
        self
    }

    /// Limits the number of tree rows in every format.
    #[must_use]
    pub const fn with_max_tree_rows(mut self, rows: usize) -> Self {
        self.max_tree_rows = Some(rows);
        self
    }

    /// Limits tree depth for the HTML rendering.
    #[must_use]
    pub const fn with_html_tree_depth(mut self, depth: usize) -> Self {
        self.html_tree_depth = Some(depth);
        self
    }

    /// Sections present in the report, in document order.
    #[must_use]
    pub fn sections(&self) -> Vec<SectionName> {
        SectionName::ALL
            .into_iter()
            .filter(|section| {
                self.include.contains(section)
                    && match section {
                        SectionName::Tree => self.tree.is_some(),
                        SectionName::Top => self.top.is_some(),
                        SectionName::Types => self.types.is_some(),
                        SectionName::Duplicates => self.duplicates.is_some(),
                        SectionName::Owners => self.owners.is_some(),
                    }
            })
            .collect()
    }

    /// Rows in one section.
    #[must_use]
    pub fn row_count(&self, section: SectionName) -> usize {
        match section {
            SectionName::Tree => self.tree_rows().count(),
            SectionName::Top => self.top.map_or(0, <[TopFile]>::len),
            SectionName::Types => self.types.map_or(0, <[TypeGroup]>::len),
            SectionName::Duplicates => self.duplicates.map_or(0, |groups| {
                groups.iter().map(|group| group.members.len()).sum()
            }),
            SectionName::Owners => self.owners.map_or(0, <[OwnerSummary]>::len),
        }
    }

    /// Total number of rows across every included section.
    #[must_use]
    pub fn total_rows(&self) -> usize {
        self.sections()
            .into_iter()
            .map(|section| self.row_count(section))
            .fold(0, usize::saturating_add)
    }

    /// Streams tree rows in node order.
    pub fn tree_rows(&self) -> impl Iterator<Item = TreeRow> + '_ {
        let limit = self.max_tree_rows.unwrap_or(usize::MAX);
        self.tree.into_iter().flat_map(move |tree| {
            (0..tree.len())
                .filter_map(move |index| NodeId::try_from(index).ok())
                .filter(move |id| !tree.node(*id).is_removed())
                .take(limit)
                .map(move |id| self.tree_row(tree, id))
        })
    }

    /// Streams HTML-limited tree rows down to the configured depth.
    pub fn html_tree_rows(&self) -> impl Iterator<Item = TreeRow> + '_ {
        let depth = self.html_tree_depth.unwrap_or(usize::MAX);
        let limit = self.max_tree_rows.unwrap_or(usize::MAX);
        self.tree.into_iter().flat_map(move |tree| {
            (0..tree.len())
                .filter_map(move |index| NodeId::try_from(index).ok())
                .filter(move |id| !tree.node(*id).is_removed() && tree.depth(*id) <= depth)
                .take(limit)
                .map(move |id| self.tree_row(tree, id))
        })
    }

    /// Builds one tree row.
    fn tree_row(&self, tree: &FsTree, id: NodeId) -> TreeRow {
        let node = tree.node(id);
        TreeRow {
            path: paths::display(&tree.path(id)),
            name: tree.name(id).to_owned(),
            kind: node.kind.as_str(),
            logical_size: node.size.logical,
            on_disk_size: node.size.total_on_disk(),
            physical_size: node.size.physical,
            files: node.file_count(),
            dirs: node.dir_count(),
            modified: node.timestamps.modified,
            attributes: attributes_text(node.attributes),
            owner: node
                .owner_id
                .and_then(|owner_id| self.owner_table.and_then(|table| table.get(owner_id)))
                .map(|info| info.display_name.clone())
                .unwrap_or_default(),
        }
    }

    /// Top-file rows in rank order.
    pub fn top_rows(&self) -> impl Iterator<Item = TopRow> + '_ {
        self.top.into_iter().flat_map(move |items| {
            items.iter().enumerate().map(|(index, file)| TopRow {
                rank: u32::try_from(index + 1).unwrap_or(u32::MAX),
                path: paths::display(&file.path),
                size: file.size.logical,
                modified: file.modified,
            })
        })
    }

    /// Type rows.
    pub fn type_rows(&self) -> impl Iterator<Item = TypeRow> + '_ {
        self.types.into_iter().flat_map(move |items| {
            items.iter().map(|group| TypeRow {
                extension: if group.extension.is_empty() {
                    "<none>".to_owned()
                } else {
                    group.extension.clone()
                },
                category: group.category.as_str().to_owned(),
                count: group.count,
                total_size: group.size.logical,
                share: group.share,
            })
        })
    }

    /// Duplicate rows; the reclaimable total appears once per group.
    pub fn duplicate_rows(&self) -> impl Iterator<Item = DuplicateRow> + '_ {
        self.duplicates.into_iter().flat_map(move |groups| {
            groups.iter().flat_map(move |group| {
                group
                    .members
                    .iter()
                    .enumerate()
                    .map(move |(index, member)| DuplicateRow {
                        group_id: group.id.to_string(),
                        hash_algorithm: group.algorithm.as_str().to_owned(),
                        hash: group.hash.clone(),
                        size: member.size,
                        path: paths::display(&member.path),
                        keep: group.keep_node_id == Some(member.node_id),
                        selected: member.selected_for_deletion,
                        reclaimable_bytes: if index == 0 {
                            group.reclaimable_bytes
                        } else {
                            0
                        },
                    })
            })
        })
    }

    /// Owner rows.
    pub fn owner_rows(&self) -> impl Iterator<Item = OwnerRow> + '_ {
        self.owners.into_iter().flat_map(move |items| {
            items.iter().map(|owner| OwnerRow {
                owner: owner.display_name.clone(),
                sid: owner.sid.clone(),
                files: owner.file_count,
                total_size: owner.total_bytes,
            })
        })
    }

    /// Rows of one section rendered as typed cells.
    #[must_use]
    pub fn section_rows(&self, section: SectionName) -> Vec<Vec<Cell>> {
        let mut rows = Vec::new();
        self.for_each_row(section, |row| {
            rows.push(row);
            Ok::<(), ()>(())
        })
        .expect("infallible row sink");

        rows
    }

    /// Streams one section into a fallible sink.
    ///
    /// # Errors
    /// Returns the first error produced by `sink`.
    pub fn for_each_row<E>(
        &self,
        section: SectionName,
        mut sink: impl FnMut(Vec<Cell>) -> Result<(), E>,
    ) -> Result<(), E> {
        match section {
            SectionName::Tree => {
                for row in self.tree_rows() {
                    sink(row.cells())?;
                }
            }
            SectionName::Top => {
                for row in self.top_rows() {
                    sink(row.cells())?;
                }
            }
            SectionName::Types => {
                for row in self.type_rows() {
                    sink(row.cells())?;
                }
            }
            SectionName::Duplicates => {
                for row in self.duplicate_rows() {
                    sink(row.cells())?;
                }
            }
            SectionName::Owners => {
                for row in self.owner_rows() {
                    sink(row.cells())?;
                }
            }
        }

        Ok(())
    }

    /// Path of a row for tests and diagnostics.
    #[must_use]
    pub fn tree_paths(&self) -> Vec<PathBuf> {
        self.tree_rows()
            .map(|row| PathBuf::from(row.path))
            .collect()
    }
}

/// Renders the attribute mask as a stable, comma-separated list.
#[must_use]
pub fn attributes_text(attributes: FileAttributes) -> String {
    let mut names = Vec::new();
    for (flag, name) in [
        (FileAttributes::READ_ONLY, "read-only"),
        (FileAttributes::HIDDEN, "hidden"),
        (FileAttributes::SYSTEM, "system"),
        (FileAttributes::DIRECTORY, "directory"),
        (FileAttributes::ARCHIVE, "archive"),
        (FileAttributes::SPARSE_FILE, "sparse"),
        (FileAttributes::REPARSE_POINT, "reparse-point"),
        (FileAttributes::COMPRESSED, "compressed"),
    ] {
        if attributes.contains(flag) {
            names.push(name);
        }
    }

    names.join(",")
}

#[cfg(test)]
mod tests {
    use freespace_analysis::DuplicateMember;
    use freespace_core::{
        size::SizeMetrics,
        tree::{AccessState, FsTree, NodeKind, NodeSpec},
    };

    use super::{Cell, Report, ReportMeta, SectionName, attributes_text};

    fn tree() -> FsTree {
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: r"C:\".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "payload.bin".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(128),
                access: AccessState::Ok,
                ..NodeSpec::default()
            },
        )
        .expect("file");
        tree.recompute_totals();

        tree
    }

    #[test]
    fn includes_only_available_sections() {
        let tree = tree();
        let report = Report::new(ReportMeta::default(), tree.node(0).size)
            .with_tree(&tree)
            .with_include(vec![SectionName::Tree, SectionName::Owners]);

        assert_eq!(report.sections(), vec![SectionName::Tree]);
        assert_eq!(report.row_count(SectionName::Tree), 2);
        assert_eq!(report.total_rows(), 2);
        assert_eq!(report.tree_rows().next().expect("root").kind, "directory");
    }

    #[test]
    fn renders_attributes_stably() {
        let mut attributes = freespace_core::tree::FileAttributes::empty();
        attributes.insert(freespace_core::tree::FileAttributes::HIDDEN);
        attributes.insert(freespace_core::tree::FileAttributes::ARCHIVE);

        assert_eq!(attributes_text(attributes), "hidden,archive");
        assert_eq!(
            attributes_text(freespace_core::tree::FileAttributes::empty()),
            ""
        );
    }

    #[test]
    fn serialises_cells_as_text() {
        assert_eq!(Cell::Unsigned(42).to_text(), "42");
        assert_eq!(Cell::Float(0.25).to_text(), "0.250000");
        assert_eq!(Cell::Bool(true).to_text(), "true");
        assert_eq!(Cell::Text("x".to_owned()).to_text(), "x");
    }

    #[test]
    fn duplicate_rows_carry_the_group_total_once() {
        let group = freespace_analysis::DuplicateGroup {
            id: freespace_core::Uuid::nil(),
            algorithm: freespace_analysis::HashAlgorithm::Sha256,
            hash: "abc".to_owned(),
            size: 10,
            members: vec![
                DuplicateMember {
                    node_id: 1,
                    path: "first".into(),
                    size: 10,
                    modified: freespace_core::Timestamp::UNIX_EPOCH,
                    created: freespace_core::Timestamp::UNIX_EPOCH,
                    selected_for_deletion: false,
                    verified: true,
                    reason: "keep".to_owned(),
                },
                DuplicateMember {
                    node_id: 2,
                    path: "second".into(),
                    size: 10,
                    modified: freespace_core::Timestamp::UNIX_EPOCH,
                    created: freespace_core::Timestamp::UNIX_EPOCH,
                    selected_for_deletion: true,
                    verified: true,
                    reason: "delete".to_owned(),
                },
            ],
            keep_node_id: Some(1),
            reclaimable_bytes: 10,
            stale: false,
        };
        let groups = [group];
        let report = Report::new(ReportMeta::default(), SizeMetrics::ZERO).with_duplicates(&groups);

        let rows: Vec<_> = report.duplicate_rows().collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].reclaimable_bytes, 10);
        assert_eq!(rows[1].reclaimable_bytes, 0);
        assert!(rows[0].keep);
        assert!(rows[1].selected);
    }
}
