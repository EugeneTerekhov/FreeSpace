//! Treemap aggregation and nesting.
//!
//! Input is an immutable snapshot and a tile budget. Directories larger than the
//! minimum tile edge are subdivided until `max_depth`; every other child becomes
//! a leaf tile. Children below the minimum visible area are merged into one
//! aggregate tile so the number of rectangles and the payload stay bounded.

use freespace_core::tree::NodeId;

use crate::{
    category,
    dto::{TreemapRect, TreemapRequest, TreemapResponse, TreemapTile},
    error::{UiError, UiErrorCode},
    tree::TreeSnapshot,
    treemap::layout::{Rect, squarify},
    validation::validate_treemap_request,
};

/// Synthetic node identifier of the aggregate tile.
const AGGREGATE_ID: NodeId = u32::MAX;

/// Builds the tiles of one Treemap request.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed request and `E_STALE_NODE` when the
/// requested node does not belong to the generation.
pub fn tiles(
    snapshot: &TreeSnapshot,
    request: &TreemapRequest,
) -> Result<TreemapResponse, UiError> {
    validate_treemap_request(request)?;

    let parent = request
        .node_id
        .or_else(|| snapshot.root_id())
        .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details("the tree is empty"))?;
    snapshot.node_row(parent)?;

    let total_size = request.metric.of(snapshot.tree().node(parent).size);
    let mut tiles = Vec::new();
    let mut truncated = false;

    if total_size > 0 {
        let bounds = Rect::new(0.0, 0.0, request.width as f32, request.height as f32);
        let mut budget = usize::from(request.max_tiles);
        add_level(
            snapshot,
            parent,
            bounds,
            0,
            request,
            &mut tiles,
            &mut budget,
            &mut truncated,
        );
    }

    Ok(TreemapResponse {
        generation: snapshot.generation(),
        tiles,
        truncated,
        total_size,
    })
}

#[allow(clippy::too_many_arguments)]
fn add_level(
    snapshot: &TreeSnapshot,
    parent: NodeId,
    bounds: Rect,
    depth: u8,
    request: &TreemapRequest,
    tiles: &mut Vec<TreemapTile>,
    budget: &mut usize,
    truncated: &mut bool,
) {
    if *budget == 0 {
        *truncated = true;
        return;
    }

    let children = snapshot.sorted_children(parent, request.metric);
    if children.is_empty() {
        return;
    }

    let parent_total: u64 = children.iter().map(|(_, size)| *size).sum();
    if parent_total == 0 {
        return;
    }

    let min_area = f64::from(request.min_tile_px) * f64::from(request.min_tile_px);
    let viewport_area = f64::from(bounds.width) * f64::from(bounds.height);
    let mut visible = Vec::new();
    let mut aggregated_size = 0u64;
    let mut hidden = 0u32;

    for (id, size) in children {
        let area = viewport_area * (size as f64 / parent_total as f64);
        let reserve = visible.is_empty() && *budget == 1;
        if area < min_area || reserve {
            aggregated_size = aggregated_size.saturating_add(size);
            hidden = hidden.saturating_add(1);
        } else {
            visible.push((id, size));
        }
    }

    let extra_aggregate = usize::from(aggregated_size > 0);
    let capacity = budget.saturating_sub(extra_aggregate).max(1);
    if visible.len() > capacity {
        for (_, size) in visible.split_off(capacity) {
            aggregated_size = aggregated_size.saturating_add(size);
            hidden = hidden.saturating_add(1);
        }
    }

    let mut items: Vec<(NodeId, f64)> = visible
        .iter()
        .map(|(id, size)| (*id, *size as f64))
        .collect();
    if aggregated_size > 0 {
        items.push((AGGREGATE_ID, aggregated_size as f64));
    }

    let laid_out = squarify(&items, bounds);
    let min_label_area = f64::from(request.min_tile_px.max(4)) * 16.0;

    for item in laid_out {
        if *budget == 0 {
            *truncated = true;
            break;
        }

        let is_aggregate = item.node == AGGREGATE_ID;
        let size = if is_aggregate {
            aggregated_size
        } else {
            visible
                .iter()
                .find(|(id, _)| *id == item.node)
                .map_or(0, |(_, size)| *size)
        };
        let fraction = (size as f64 / parent_total as f64) as f32;
        let rect = TreemapRect {
            x: item.rect.x,
            y: item.rect.y,
            w: item.rect.width,
            h: item.rect.height,
        };

        if is_aggregate {
            push_tile(
                tiles,
                budget,
                TreemapTile {
                    node_id: None,
                    rect,
                    depth,
                    size,
                    fraction,
                    color_key: "other".to_owned(),
                    label: String::new(),
                    is_aggregate: true,
                    child_count: hidden,
                },
            );
            *truncated = true;
            continue;
        }

        let node = snapshot.tree().node(item.node);
        let has_children =
            node.is_directory() && snapshot.tree().children(item.node).next().is_some();
        let can_subdivide = has_children
            && depth + 1 < request.max_depth
            && *budget > 1
            && f64::from(rect.w) * f64::from(rect.h) >= min_area;

        if can_subdivide {
            let before = tiles.len();
            add_level(
                snapshot,
                item.node,
                item.rect,
                depth + 1,
                request,
                tiles,
                budget,
                truncated,
            );
            if tiles.len() > before {
                continue;
            }
        }

        let area = f64::from(rect.w) * f64::from(rect.h);
        let label = if area >= min_label_area {
            truncate_label(snapshot.tree().name(item.node))
        } else {
            String::new()
        };
        let child_count =
            u32::try_from(snapshot.tree().children(item.node).count()).unwrap_or(u32::MAX);
        let color_key =
            category::node_category_key(snapshot.tree().name(item.node), node.is_directory());

        push_tile(
            tiles,
            budget,
            TreemapTile {
                node_id: Some(item.node),
                rect,
                depth,
                size,
                fraction,
                color_key: color_key.to_owned(),
                label,
                is_aggregate: false,
                child_count,
            },
        );
    }
}

fn push_tile(tiles: &mut Vec<TreemapTile>, budget: &mut usize, tile: TreemapTile) {
    if *budget > 0 {
        tiles.push(tile);
        *budget -= 1;
    }
}

fn truncate_label(name: &str) -> String {
    const MAX_CHARS: usize = 64;

    if name.chars().count() <= MAX_CHARS {
        return name.to_owned();
    }

    let mut label: String = name.chars().take(MAX_CHARS.saturating_sub(1)).collect();
    label.push('…');

    label
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        model::{FileSystem, Volume, VolumeStatus},
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::tiles;
    use crate::{
        dto::{Metric, TreemapRequest, TreemapTile},
        error::UiErrorCode,
        tree::TreeSnapshot,
    };

    fn snapshot() -> TreeSnapshot {
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: "root".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "video.mp4".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(500),
                ..NodeSpec::default()
            },
        )
        .expect("video");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "photo.jpg".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(300),
                ..NodeSpec::default()
            },
        )
        .expect("photo");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "note.txt".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(200),
                ..NodeSpec::default()
            },
        )
        .expect("note");
        tree.recompute_totals();

        let volume = Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(r"C:\")),
            mount_point: r"C:\".into(),
            label: "Test".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 10_000,
            free_bytes: 0,
            cluster_size: Some(4096),
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        };

        TreeSnapshot::new(tree, 1, volume, None)
    }

    fn request() -> TreemapRequest {
        TreemapRequest {
            width: 400,
            height: 300,
            max_tiles: 100,
            max_depth: 2,
            min_tile_px: 4,
            metric: Metric::OnDisk,
            ..TreemapRequest::default()
        }
    }

    #[test]
    fn lays_out_children_without_overlaps() {
        let snapshot = snapshot();
        let response = tiles(&snapshot, &request()).expect("tiles");

        assert_eq!(response.total_size, 1_000);
        assert_eq!(response.tiles.len(), 3);
        assert!(!response.truncated);
        let sum: u64 = response.tiles.iter().map(|tile| tile.size).sum();
        assert_eq!(sum, 1_000);

        for (index, left) in response.tiles.iter().enumerate() {
            assert!(left.rect.w > 0.0 && left.rect.h > 0.0);
            assert!(
                left.rect.x >= 0.0
                    && left.rect.y >= 0.0
                    && left.rect.x + left.rect.w <= 400.0 + 0.01
                    && left.rect.y + left.rect.h <= 300.0 + 0.01
            );
            for right in response.tiles.iter().skip(index + 1) {
                assert!(!overlaps(left, right), "{left:?} overlaps {right:?}");
            }
        }

        let video = response
            .tiles
            .iter()
            .find(|tile| tile.label == "video.mp4")
            .expect("video tile");
        assert_eq!(video.color_key, "video");
        assert_eq!(video.depth, 0);
        assert_eq!(video.fraction, 0.5);
    }

    #[test]
    fn merges_small_children_into_an_aggregate() {
        let snapshot = snapshot();
        let response = tiles(
            &snapshot,
            &TreemapRequest {
                width: 20,
                height: 20,
                min_tile_px: 40,
                ..request()
            },
        )
        .expect("tiles");

        assert_eq!(response.tiles.len(), 1);
        assert!(response.tiles[0].is_aggregate);
        assert!(response.truncated);
        assert_eq!(response.tiles[0].size, 1_000);
    }

    #[test]
    fn respects_the_tile_budget() {
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: "root".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        for index in 0..150u32 {
            tree.add_node(
                Some(root),
                NodeSpec {
                    name: format!("file-{index:03}.bin"),
                    kind: NodeKind::File,
                    size: SizeMetrics::file(u64::from(index + 1)),
                    ..NodeSpec::default()
                },
            )
            .expect("file");
        }
        tree.recompute_totals();
        let volume = snapshot().volume().clone();
        let snapshot = TreeSnapshot::new(tree, 1, volume, None);

        let response = tiles(
            &snapshot,
            &TreemapRequest {
                max_tiles: 100,
                width: 1_000,
                height: 1_000,
                min_tile_px: 2,
                ..request()
            },
        )
        .expect("tiles");

        assert!(response.tiles.len() <= 100);
        assert!(response.truncated);
        assert!(response.tiles.iter().any(|tile| tile.is_aggregate));
    }

    #[test]
    fn subdivides_directories_within_max_depth() {
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: "root".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        let folder = tree
            .add_node(
                Some(root),
                NodeSpec {
                    name: "folder".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("folder");
        tree.add_node(
            Some(folder),
            NodeSpec {
                name: "deep.bin".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(400),
                ..NodeSpec::default()
            },
        )
        .expect("deep");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "other.bin".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(600),
                ..NodeSpec::default()
            },
        )
        .expect("other");
        tree.recompute_totals();
        let snapshot = TreeSnapshot::new(tree, 1, snapshot().volume().clone(), None);

        let response = tiles(&snapshot, &request()).expect("tiles");
        let deep = response.tiles.iter().find(|tile| tile.label == "deep.bin");
        assert!(deep.is_some(), "{:?}", response.tiles);
        assert_eq!(deep.expect("deep").depth, 1);

        let shallow = tiles(
            &snapshot,
            &TreemapRequest {
                max_depth: 1,
                ..request()
            },
        )
        .expect("shallow");
        assert!(shallow.tiles.iter().all(|tile| tile.depth == 0));
        assert!(shallow.tiles.iter().any(|tile| tile.label == "folder"));
    }

    #[test]
    fn rejects_unknown_nodes() {
        let snapshot = snapshot();
        let error = tiles(
            &snapshot,
            &TreemapRequest {
                node_id: Some(999),
                ..request()
            },
        )
        .expect_err("stale");

        assert_eq!(error.code, UiErrorCode::StaleNode);
    }

    fn overlaps(left: &TreemapTile, right: &TreemapTile) -> bool {
        left.rect.x < right.rect.x + right.rect.w - 0.01
            && right.rect.x < left.rect.x + left.rect.w - 0.01
            && left.rect.y < right.rect.y + right.rect.h - 0.01
            && right.rect.y < left.rect.y + left.rect.h - 0.01
    }
}
