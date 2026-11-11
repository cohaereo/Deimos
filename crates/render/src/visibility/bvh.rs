use std::range::Range;

use deimos_data::tfx::{features::dynamic::RenderStageSubscription, geometry::AxisAlignedBBox};
use smallvec::SmallVec;

use crate::visibility::{ViewVisibility, frustum::AabbVisibility};

#[derive(Debug, Clone)]
struct FlatNode {
    aabb: AxisAlignedBBox,
    data: NodeData,
}

#[derive(Debug, Clone)]
enum NodeData {
    Internal { right_child: u32 },
    Leaf { item_range: Range<u32> },
}

#[derive(Debug, Clone, Copy)]
struct LeafItem {
    original_index: u32,
    aabb: AxisAlignedBBox,
}

pub struct Bvh {
    nodes: Vec<FlatNode>,
    items: Vec<LeafItem>,
}

const LEAF_SIZE: usize = 8;

impl Bvh {
    pub fn build(aabbs: &[AxisAlignedBBox]) -> Self {
        assert!(!aabbs.is_empty(), "Cannot build BVH from empty slice");

        let mut work: Vec<u32> = (0..aabbs.len() as u32).collect();
        let mut nodes = Vec::with_capacity(2 * aabbs.len() / LEAF_SIZE + 1);
        let mut items = Vec::with_capacity(aabbs.len());

        build_recursive(aabbs, &mut work, &mut nodes, &mut items);
        debug_assert_eq!(items.len(), aabbs.len());

        Self { nodes, items }
    }

    #[profiling::function]
    pub fn has_visible_leaves(&self, vis: &ViewVisibility) -> bool {
        if self.nodes.is_empty() {
            return false;
        }

        let mut stack = SmallVec::<[u32; 64]>::new();
        stack.push(0u32);

        while let Some(idx) = stack.pop() {
            let node = &self.nodes[idx as usize];

            match node.data {
                NodeData::Internal { right_child } => {
                    if vis.is_visible_frustum(&node.aabb) {
                        stack.push(right_child);
                        stack.push(idx + 1);
                    }
                }
                NodeData::Leaf { item_range } => {
                    let start = item_range.start as usize;
                    let end = item_range.end as usize;
                    let items = &self.items[start..end];
                    if items.iter().any(|item| vis.is_visible(&item.aabb)) {
                        return true;
                    }
                }
            }
        }
        false
    }

    #[profiling::function]
    pub fn collect_visible_leaves(&self, vis: &ViewVisibility, out: &mut Vec<u32>, offset: u32) {
        if self.nodes.is_empty() {
            return;
        }

        let is_shadow_view =
            vis.render_stages.bits() == RenderStageSubscription::SHADOW_VIEW.bits();

        let mut stack = SmallVec::<[(u32, bool); 64]>::new();
        stack.push((0u32, false));

        while let Some((idx, inside)) = stack.pop() {
            let node = &self.nodes[idx as usize];

            match node.data {
                NodeData::Internal { right_child } => {
                    if inside {
                        stack.push((right_child, true));
                        stack.push((idx + 1, true));
                    } else if is_shadow_view {
                        match vis.culling_frustum.aabb_test(&node.aabb) {
                            AabbVisibility::Outside => {}
                            AabbVisibility::Intersecting => {
                                stack.push((right_child, false));
                                stack.push((idx + 1, false));
                            }
                            AabbVisibility::Inside => {
                                stack.push((right_child, true));
                                stack.push((idx + 1, true));
                            }
                        }
                    } else if vis.is_visible_frustum(&node.aabb) {
                        stack.push((right_child, false));
                        stack.push((idx + 1, false));
                    }
                }
                NodeData::Leaf { item_range } => {
                    let start = item_range.start as usize;
                    let end = item_range.end as usize;
                    for item in &self.items[start..end] {
                        let visible = if inside {
                            vis.is_visible_projected(&item.aabb)
                        } else {
                            vis.is_visible(&item.aabb)
                        };
                        if visible {
                            out.push(offset + item.original_index);
                        }
                    }
                }
            }
        }
    }

    pub const fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn depth(&self) -> usize {
        fn recurse(nodes: &[FlatNode], idx: usize) -> usize {
            match nodes[idx].data {
                NodeData::Leaf { .. } => 1,
                NodeData::Internal { right_child } => {
                    let l = recurse(nodes, idx + 1);
                    let r = recurse(nodes, right_child as usize);
                    1 + l.max(r)
                }
            }
        }
        if self.nodes.is_empty() {
            0
        } else {
            recurse(&self.nodes, 0)
        }
    }
}

fn merged_aabb(aabbs: &[AxisAlignedBBox], work: &[u32]) -> AxisAlignedBBox {
    work.iter()
        .skip(1)
        .fold(aabbs[work[0] as usize], |acc, &i| {
            acc.union(&aabbs[i as usize])
        })
}

fn build_recursive(
    aabbs: &[AxisAlignedBBox],
    work: &mut [u32],
    nodes: &mut Vec<FlatNode>,
    items: &mut Vec<LeafItem>,
) {
    let aabb = merged_aabb(aabbs, work);

    if work.len() <= LEAF_SIZE {
        let first_item = items.len() as u32;
        items.extend(work.iter().map(|&i| LeafItem {
            original_index: i,
            aabb: aabbs[i as usize],
        }));
        nodes.push(FlatNode {
            aabb,
            data: NodeData::Leaf {
                item_range: (first_item..(first_item + work.len() as u32)).into(),
            },
        });
        return;
    }

    let axis = aabb.longest_axis();
    work.sort_unstable_by(|&a, &b| {
        aabbs[a as usize].centroid()[axis]
            .partial_cmp(&aabbs[b as usize].centroid()[axis])
            .unwrap()
    });

    let mid = work.len() / 2;

    let this_index = nodes.len();
    nodes.push(FlatNode {
        aabb,
        data: NodeData::Internal { right_child: 0 },
    });

    build_recursive(aabbs, &mut work[..mid], nodes, items);

    let right_child = nodes.len() as u32;
    nodes[this_index].data = NodeData::Internal { right_child };

    build_recursive(aabbs, &mut work[mid..], nodes, items);
}
