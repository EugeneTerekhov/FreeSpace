//! Search, filtering and sorting over [`FsTree`](crate::tree::FsTree).
//!
//! The tree is the single source of truth for every screen: the size tree, the
//! treemap, the CLI `tree` command and the top-file list all need the same
//! subset of nodes in the same order. Keeping the rules here guarantees that
//! every frontend answers identically (FR-007, FR-008).

use std::path::Path;

use globset::{GlobBuilder, GlobMatcher};
use serde::{Deserialize, Serialize};

use crate::{
    Error, Result,
    size::SizeMetrics,
    time::Timestamp,
    tree::{FileAttributes, FsTree, NodeId, NodeKind},
};

/// Field used to order sibling nodes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortKey {
    /// File name, case-insensitive.
    Name,
    /// Aggregated logical size.
    #[default]
    Size,
    /// Last modification time.
    Modified,
    /// Object kind and extension.
    Type,
    /// Owner index.
    Owner,
    /// Extension, used by metadata-oriented screens.
    Extension,
}

impl SortKey {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Size => "size",
            Self::Modified => "modified",
            Self::Type => "type",
            Self::Owner => "owner",
            Self::Extension => "extension",
        }
    }

    /// Parses a user-supplied key.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "name" => Some(Self::Name),
            "size" => Some(Self::Size),
            "modified" | "date" | "mtime" => Some(Self::Modified),
            "type" | "kind" => Some(Self::Type),
            "owner" => Some(Self::Owner),
            "extension" | "ext" => Some(Self::Extension),
            _ => None,
        }
    }
}

/// Direction of a sort.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    /// Smallest or first alphabetically first.
    Ascending,
    /// Largest or last alphabetically first.
    #[default]
    Descending,
}

impl SortDirection {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }
}

/// A sort key plus a direction.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SortSpec {
    /// Field to order by.
    pub key: SortKey,
    /// Direction of the order.
    pub direction: SortDirection,
}

impl SortSpec {
    /// Builds a full specification.
    #[must_use]
    pub const fn new(key: SortKey, direction: SortDirection) -> Self {
        Self { key, direction }
    }

    /// Sorts node ids in place using the tree data.
    pub fn sort(&self, tree: &FsTree, ids: &mut [NodeId]) {
        let descending = matches!(self.direction, SortDirection::Descending);

        match self.key {
            SortKey::Name => ids.sort_by_cached_key(|id| {
                SortKeyValue::new(tree.name(*id).to_lowercase(), descending)
            }),
            SortKey::Size => ids.sort_by_cached_key(|id| {
                SortKeyValue::new(tree.node(*id).size.logical, descending)
            }),
            SortKey::Modified => ids.sort_by_cached_key(|id| {
                SortKeyValue::new(tree.node(*id).timestamps.modified, descending)
            }),
            SortKey::Type | SortKey::Extension => ids.sort_by_cached_key(|id| {
                let extension = extension_of(tree.name(*id)).to_lowercase();
                let kind = tree.node(*id).kind.as_str();
                SortKeyValue::new(format!("{kind}/{extension}"), descending)
            }),
            SortKey::Owner => {
                ids.sort_by_cached_key(|id| SortKeyValue::new(tree.node(*id).owner_id, descending))
            }
        }
    }
}

/// Key wrapper that applies the sort direction inside `sort_by_cached_key`.
#[derive(Debug, PartialEq, Eq)]
struct SortKeyValue<T: Ord> {
    value: T,
    descending: bool,
}

impl<T: Ord> SortKeyValue<T> {
    fn new(value: T, descending: bool) -> Self {
        Self { value, descending }
    }
}

impl<T: Ord> Ord for SortKeyValue<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let ordering = self.value.cmp(&other.value);
        if self.descending {
            ordering.reverse()
        } else {
            ordering
        }
    }
}

impl<T: Ord> PartialOrd for SortKeyValue<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Declarative description of a filter; safe to store and serialise.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterSpec {
    /// Name glob matched against the file name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Lower-case extensions without the leading dot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
    /// Accepted object kinds; empty means every kind.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<NodeKind>,
    /// Minimum aggregated logical size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_size: Option<u64>,
    /// Maximum aggregated logical size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<u64>,
    /// Only nodes modified at or before this instant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_before: Option<Timestamp>,
    /// Only nodes modified at or after this instant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_after: Option<Timestamp>,
    /// Whether hidden and system objects are included.
    #[serde(default)]
    pub include_hidden: bool,
}

impl FilterSpec {
    /// Compiles the specification into a reusable matcher.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] when the glob is malformed.
    pub fn compile(&self) -> Result<NodeFilter> {
        let matcher = match &self.pattern {
            Some(pattern) if !pattern.is_empty() => Some(
                GlobBuilder::new(pattern)
                    .case_insensitive(true)
                    .literal_separator(false)
                    .backslash_escape(false)
                    .build()
                    .map_err(|error| {
                        Error::InvalidArgument(format!("invalid glob `{pattern}`: {error}"))
                    })?
                    .compile_matcher(),
            ),
            _ => None,
        };

        Ok(NodeFilter {
            spec: self.clone(),
            matcher,
        })
    }
}

/// A compiled filter.
#[derive(Debug, Clone)]
pub struct NodeFilter {
    spec: FilterSpec,
    matcher: Option<GlobMatcher>,
}

impl NodeFilter {
    /// Original specification.
    #[must_use]
    pub const fn spec(&self) -> &FilterSpec {
        &self.spec
    }

    /// Returns `true` when the node satisfies every constraint.
    #[must_use]
    pub fn matches(&self, tree: &FsTree, id: NodeId) -> bool {
        let node = tree.node(id);
        if node.is_removed() {
            return false;
        }
        if !self.spec.include_hidden
            && (node.attributes.contains(FileAttributes::HIDDEN)
                || node.attributes.contains(FileAttributes::SYSTEM))
        {
            return false;
        }
        if !self.spec.kinds.is_empty() && !self.spec.kinds.contains(&node.kind) {
            return false;
        }

        let size = node.size.logical;
        if self.spec.min_size.is_some_and(|minimum| size < minimum) {
            return false;
        }
        if self.spec.max_size.is_some_and(|maximum| size > maximum) {
            return false;
        }

        let modified = node.timestamps.modified;
        if self
            .spec
            .modified_before
            .is_some_and(|limit| modified > limit)
        {
            return false;
        }
        if self
            .spec
            .modified_after
            .is_some_and(|limit| modified < limit)
        {
            return false;
        }

        if !self.spec.extensions.is_empty() {
            let extension = extension_of(tree.name(id)).to_ascii_lowercase();
            if extension.is_empty()
                || !self
                    .spec
                    .extensions
                    .iter()
                    .any(|accepted| accepted.eq_ignore_ascii_case(&extension))
            {
                return false;
            }
        }

        match &self.matcher {
            Some(matcher) => matcher.is_match(Path::new(tree.name(id))),
            None => true,
        }
    }
}

/// Returns the sorted direct children of `parent` that satisfy `filter`.
#[must_use]
pub fn filtered_children(
    tree: &FsTree,
    parent: NodeId,
    filter: &NodeFilter,
    sort: SortSpec,
) -> Vec<NodeId> {
    let mut ids: Vec<NodeId> = tree
        .children(parent)
        .filter(|id| filter.matches(tree, *id))
        .collect();
    sort.sort(tree, &mut ids);

    ids
}

/// Depth-first search below `root`.
///
/// Matches are returned in the requested order within every directory. When
/// `with_ancestors` is set, the ancestors of every match are included so that a
/// filtered tree can still be rendered as a tree (acceptance scenario 5 of
/// US1).
#[must_use]
pub fn search(
    tree: &FsTree,
    root: NodeId,
    filter: &NodeFilter,
    sort: SortSpec,
    with_ancestors: bool,
    limit: usize,
) -> Vec<NodeId> {
    let mut results = Vec::new();
    let mut seen = vec![false; tree.len()];

    walk(
        tree,
        root,
        filter,
        sort,
        with_ancestors,
        limit,
        &mut results,
        &mut seen,
    );

    if with_ancestors {
        // The walk collects matches bottom-up so that ancestors can be marked;
        // reverse the result so parents are listed before their children.
        results.reverse();
    }

    results
}

#[allow(clippy::too_many_arguments)]
fn walk(
    tree: &FsTree,
    id: NodeId,
    filter: &NodeFilter,
    sort: SortSpec,
    with_ancestors: bool,
    limit: usize,
    results: &mut Vec<NodeId>,
    seen: &mut [bool],
) -> bool {
    if results.len() >= limit {
        return true;
    }

    let mut matched_self = filter.matches(tree, id);
    let mut children: Vec<NodeId> = tree.children(id).collect();
    sort.sort(tree, &mut children);
    let mut child_matched = false;
    for child in children {
        child_matched |= walk(
            tree,
            child,
            filter,
            sort,
            with_ancestors,
            limit,
            results,
            seen,
        );
        if results.len() >= limit {
            break;
        }
    }

    let include = matched_self || (with_ancestors && child_matched);
    if include && !seen[id as usize] && results.len() < limit {
        seen[id as usize] = true;
        results.push(id);
    }

    matched_self |= child_matched;
    matched_self
}

/// Extension of a file name, without the dot.
#[must_use]
pub fn extension_of(name: &str) -> &str {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
}

/// Sums the sizes of the given nodes.
#[must_use]
pub fn total_size(tree: &FsTree, ids: &[NodeId]) -> SizeMetrics {
    ids.iter().map(|id| tree.node(*id).size).sum()
}

#[cfg(test)]
mod tests {
    use crate::{
        size::SizeMetrics,
        time::Timestamp,
        tree::{AccessState, FileAttributes, FsTree, NodeKind, NodeSpec, Timestamps},
    };

    use super::{
        FilterSpec, SortDirection, SortKey, SortSpec, extension_of, filtered_children, search,
    };

    fn file(name: &str, bytes: u64) -> NodeSpec {
        NodeSpec {
            name: name.to_owned(),
            kind: NodeKind::File,
            size: SizeMetrics::file(bytes),
            ..NodeSpec::default()
        }
    }

    fn fixture() -> (FsTree, u32, Vec<u32>) {
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: r"C:\data".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        let mut documents = file("report.txt", 100);
        documents.timestamps = Timestamps {
            modified: Timestamp::from_millis(1_000),
            ..Timestamps::default()
        };
        let mut hidden = file("secret.txt", 50);
        hidden.attributes.insert(FileAttributes::HIDDEN);

        let note = tree.add_node(Some(root), documents).expect("note");
        let hidden = tree.add_node(Some(root), hidden).expect("hidden");
        let image = tree
            .add_node(Some(root), file("photo.JPG", 400))
            .expect("image");
        let folder = tree
            .add_node(
                Some(root),
                NodeSpec {
                    name: "nested".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("nested");
        let deep = tree
            .add_node(Some(folder), file("deep.txt", 25))
            .expect("deep");
        tree.recompute_totals();

        (tree, root, vec![note, hidden, image, folder, deep])
    }

    #[test]
    fn sorts_by_size_and_name_both_ways() {
        let (tree, root, _) = fixture();
        let filter = FilterSpec::default().compile().expect("filter");

        let by_size = filtered_children(
            &tree,
            root,
            &filter,
            SortSpec::new(SortKey::Size, SortDirection::Descending),
        );
        assert_eq!(tree.name(by_size[0]), "photo.JPG");
        assert_eq!(tree.name(by_size[1]), "report.txt");

        let by_name = filtered_children(
            &tree,
            root,
            &filter,
            SortSpec::new(SortKey::Name, SortDirection::Ascending),
        );
        assert_eq!(tree.name(by_name[0]), "nested");
        assert_eq!(tree.name(by_name[2]), "report.txt");
        assert_eq!(by_name.len(), 3, "hidden files stay filtered out");
    }

    #[test]
    fn filters_by_glob_extension_size_and_visibility() {
        let (tree, root, ids) = fixture();

        let glob = FilterSpec {
            pattern: Some("*.txt".to_owned()),
            ..FilterSpec::default()
        }
        .compile()
        .expect("glob");
        let matches = search(
            &tree,
            root,
            &glob,
            SortSpec::new(SortKey::Name, SortDirection::Ascending),
            false,
            10,
        );
        assert!(matches.contains(&ids[0]));
        assert!(matches.contains(&ids[4]));
        assert!(
            !matches.contains(&ids[3]),
            "ancestors are excluded on request"
        );

        let visible = FilterSpec {
            pattern: Some("*.txt".to_owned()),
            include_hidden: false,
            ..FilterSpec::default()
        }
        .compile()
        .expect("visible");
        assert!(!visible.spec().include_hidden);
        assert!(!visible.matches(&tree, ids[1]));

        let extensions = FilterSpec {
            extensions: vec!["jpg".to_owned()],
            ..FilterSpec::default()
        }
        .compile()
        .expect("extensions");
        assert_eq!(
            filtered_children(
                &tree,
                root,
                &extensions,
                SortSpec::new(SortKey::Name, SortDirection::Ascending)
            ),
            vec![ids[2]]
        );

        let large = FilterSpec {
            min_size: Some(100),
            ..FilterSpec::default()
        }
        .compile()
        .expect("large");
        assert!(large.matches(&tree, ids[0]));
        assert!(!large.matches(&tree, ids[1]));
    }

    #[test]
    fn searches_recursively_and_optionally_keeps_ancestors() {
        let (tree, root, ids) = fixture();
        let filter = FilterSpec {
            pattern: Some("deep.txt".to_owned()),
            ..FilterSpec::default()
        }
        .compile()
        .expect("filter");
        let sort = SortSpec::new(SortKey::Name, SortDirection::Ascending);

        let matches = search(&tree, root, &filter, sort, false, 10);
        assert_eq!(matches, vec![ids[4]]);

        let with_ancestors = search(&tree, root, &filter, sort, true, 10);
        assert_eq!(with_ancestors, vec![root, ids[3], ids[4]]);
    }

    #[test]
    fn limits_search_results() {
        let (tree, root, _) = fixture();
        let filter = FilterSpec::default().compile().expect("filter");

        let matches = search(
            &tree,
            root,
            &filter,
            SortSpec::new(SortKey::Size, SortDirection::Descending),
            false,
            2,
        );

        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn rejects_malformed_globs() {
        let filter = FilterSpec {
            pattern: Some("[".to_owned()),
            ..FilterSpec::default()
        };

        assert!(filter.compile().is_err());
    }

    #[test]
    fn extracts_extensions() {
        assert_eq!(extension_of("archive.tar.gz"), "gz");
        assert_eq!(extension_of("README"), "");
        assert_eq!(extension_of(".gitignore"), "");
    }

    #[test]
    fn skips_removed_nodes_everywhere() {
        let (mut tree, root, ids) = fixture();
        tree.mark_removed(ids[3]);
        tree.recompute_totals();
        let filter = FilterSpec::default().compile().expect("filter");
        let children = filtered_children(
            &tree,
            root,
            &filter,
            SortSpec::new(SortKey::Name, SortDirection::Ascending),
        );

        assert!(!children.contains(&ids[3]));
        assert_eq!(tree.node(root).size.logical, 550);
        assert_eq!(tree.node(root).file_count(), 3);
        assert_eq!(tree.node(root).dir_count(), 0);
        assert!(matches!(tree.node(ids[2]).access, AccessState::Ok));
    }
}
