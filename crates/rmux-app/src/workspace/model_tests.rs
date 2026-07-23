use super::*;

fn make_workspace(id: u64, name: &str, next_pane_id: &mut u64) -> Workspace {
    Workspace::new(WorkspaceId(id), name.to_string(), next_pane_id)
}

#[test]
fn test_workspace_creation() {
    let mut pane_id = 1;
    let ws = make_workspace(1, "Test", &mut pane_id);
    assert_eq!(ws.pane_count(), 1);
    assert!(!ws.pane_ids().is_empty());
}

#[test]
fn test_split_right() {
    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let original_active = ws.active_pane;
    let new_id = ws.split_right(original_active, &mut pane_id, &mut split_id).unwrap();
    assert_eq!(ws.pane_count(), 2);
    assert_ne!(new_id, original_active);
}

#[test]
fn test_close_pane_updates_focus() {
    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let p1 = ws.active_pane;
    let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();

    ws.focus_pane(p2);
    ws.close_pane(p1).unwrap();
    assert_eq!(ws.pane_count(), 1);
    assert_eq!(ws.active_pane, p2);

    let result = ws.close_pane(p2);
    assert!(result.is_err());
}

#[test]
fn test_focus_next_prev() {
    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let p1 = ws.active_pane;
    let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();
    let _p3 = ws.split_down(p2, &mut pane_id, &mut split_id).unwrap();

    let panes = ws.pane_ids();
    assert_eq!(panes.len(), 3);

    ws.focus_pane(p1);
    ws.focus_next();
    assert_eq!(ws.active_pane, panes[1]);
    ws.focus_next();
    assert_eq!(ws.active_pane, panes[2]);
    ws.focus_next();
    assert_eq!(ws.active_pane, panes[0]);

    ws.focus_prev();
    assert_eq!(ws.active_pane, panes[2]);
    ws.focus_prev();
    assert_eq!(ws.active_pane, panes[1]);
}

#[test]
fn test_focus_left_right_horizontal_split() {
    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let p1 = ws.active_pane;
    let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();

    // p1 is left, p2 is right
    ws.focus_pane(p2);
    ws.focus_left();
    assert_eq!(ws.active_pane, p1);

    ws.focus_right();
    assert_eq!(ws.active_pane, p2);
}

#[test]
fn test_focus_up_down_vertical_split() {
    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let p1 = ws.active_pane;
    let p2 = ws.split_down(p1, &mut pane_id, &mut split_id).unwrap();

    // p1 is top, p2 is bottom
    ws.focus_pane(p2);
    ws.focus_up();
    assert_eq!(ws.active_pane, p1);

    ws.focus_down();
    assert_eq!(ws.active_pane, p2);
}

#[test]
fn test_focus_spatial_nested_layout() {
    // Layout: horizontal split [p1, vertical_split[p2, p3]]
    // p1 is on the left
    // p2 is top-right
    // p3 is bottom-right
    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let p1 = ws.active_pane;
    let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();
    let p3 = ws.split_down(p2, &mut pane_id, &mut split_id).unwrap();

    // From p1 (left), right goes to p2 (top-right, vertically aligned)
    ws.focus_pane(p1);
    ws.focus_right();
    assert_eq!(ws.active_pane, p2);

    // From p2 (top-right), down goes to p3 (bottom-right)
    ws.focus_down();
    assert_eq!(ws.active_pane, p3);

    // From p3 (bottom-right), up goes to p2 (top-right)
    ws.focus_up();
    assert_eq!(ws.active_pane, p2);

    // From p2 (top-right), left goes to p1 (left)
    ws.focus_left();
    assert_eq!(ws.active_pane, p1);
}

#[test]
fn test_focus_direction_spatial_variant() {
    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let p1 = ws.active_pane;
    let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();

    ws.focus_pane(p1);
    ws.focus_direction(FocusDirection::Spatial { dx: 1, dy: 0 });
    assert_eq!(ws.active_pane, p2);

    ws.focus_direction(FocusDirection::Spatial { dx: -1, dy: 0 });
    assert_eq!(ws.active_pane, p1);
}

#[test]
fn test_focus_spatial_no_pane_in_direction() {
    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let p1 = ws.active_pane;
    let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();

    // From rightmost pane, focus_right should not change focus
    ws.focus_pane(p2);
    ws.focus_right();
    assert_eq!(ws.active_pane, p2);

    // From leftmost pane, focus_left should not change focus
    ws.focus_pane(p1);
    ws.focus_left();
    assert_eq!(ws.active_pane, p1);
}

#[test]
fn test_new_workspace_has_no_git_info_or_ports() {
    let mut pane_id = 1;
    let ws = make_workspace(1, "Test", &mut pane_id);
    assert_eq!(ws.git_branch(), None);
    assert_eq!(ws.git_status(), None);
    assert!(ws.ports().is_empty());
}

#[test]
fn test_apply_automatic_title_updates_name_when_not_custom() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Terminal", &mut pane_id);
    assert!(!ws.name_is_custom);
    assert!(ws.apply_automatic_title("cargo run -p rmux".into()));
    assert_eq!(ws.name, "cargo run -p rmux");
    assert_eq!(ws.process_title, "cargo run -p rmux");
}

#[test]
fn test_custom_name_blocks_automatic_title() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Terminal", &mut pane_id);
    ws.set_custom_name("My project".into());
    assert!(ws.name_is_custom);
    assert!(!ws.apply_automatic_title("cargo run".into()));
    assert_eq!(ws.name, "My project");
    // process_title still tracks auto source for restore
    assert_eq!(ws.process_title, "cargo run");
    ws.clear_custom_name();
    assert!(!ws.name_is_custom);
    assert_eq!(ws.name, "cargo run");
}

#[test]
fn test_update_git_info_sets_branch_and_status() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);

    ws.update_git_info(Some("main".to_string()), Some("clean".to_string()));
    assert_eq!(ws.git_branch(), Some("main"));
    assert_eq!(ws.git_status(), Some("clean"));

    // Updating again with a different status overwrites both fields.
    ws.update_git_info(Some("feature/x".to_string()), Some("modified".to_string()));
    assert_eq!(ws.git_branch(), Some("feature/x"));
    assert_eq!(ws.git_status(), Some("modified"));
}

#[test]
fn test_update_git_info_can_clear_fields() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    ws.update_git_info(Some("main".to_string()), Some("clean".to_string()));

    // Passing None clears each field independently.
    ws.update_git_info(None, None);
    assert_eq!(ws.git_branch(), None);
    assert_eq!(ws.git_status(), None);
}

#[test]
fn test_update_git_info_partial_update() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    ws.update_git_info(Some("main".to_string()), Some("clean".to_string()));

    // Update only the status; branch should be preserved? Per spec,
    // update_git_info takes both arguments, so the caller must pass the
    // current branch to keep it. Verify that behavior.
    let current_branch = ws.git_branch().map(str::to_owned);
    ws.update_git_info(current_branch, Some("modified".to_string()));
    assert_eq!(ws.git_branch(), Some("main"));
    assert_eq!(ws.git_status(), Some("modified"));
}

#[test]
fn test_update_ports_replaces_list() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);

    ws.update_ports(vec![3000, 8080]);
    assert_eq!(ws.ports(), &[3000, 8080]);

    ws.update_ports(vec![5000]);
    assert_eq!(ws.ports(), &[5000]);

    ws.update_ports(Vec::new());
    assert!(ws.ports().is_empty());
}

#[test]
fn test_ports_and_git_accessors_return_borrows() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    ws.update_git_info(Some("dev".to_string()), Some("untracked".to_string()));
    ws.update_ports(vec![443]);

    // Accessors return borrows, allowing read-only use without moving.
    let branch: Option<&str> = ws.git_branch();
    let status: Option<&str> = ws.git_status();
    let ports: &[u16] = ws.ports();
    assert_eq!(branch, Some("dev"));
    assert_eq!(status, Some("untracked"));
    assert_eq!(ports, &[443]);
}

// ====================================================================
// W2.2 — Workspace surface (tab) management methods
// ====================================================================

/// Borrow the active leaf's surface list, panicking on a non-leaf
/// (the test workspace always has a leaf at `active_pane`).
fn leaf_surfaces_of(ws: &Workspace) -> &Vec<Surface> {
    fn walk(node: &PaneNode, target: PaneId) -> Option<&Vec<Surface>> {
        match node {
            PaneNode::Leaf { id, surfaces, .. } if *id == target => Some(surfaces),
            PaneNode::Leaf { .. } | PaneNode::Browser { .. } => None,
            PaneNode::Split { children, .. } => children.iter().find_map(|c| walk(c, target)),
        }
    }
    walk(&ws.root, ws.active_pane).expect("active pane is a Leaf")
}

#[test]
fn test_workspace_new_surface_creates_with_id() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let id = ws.new_surface("Terminal A".to_string()).unwrap();
    assert_eq!(id, SurfaceId(1));
    let surfaces = leaf_surfaces_of(&ws);
    assert_eq!(surfaces.len(), 1);
    assert_eq!(surfaces[0].id, SurfaceId(1));
    assert_eq!(surfaces[0].title, "Terminal A");
    assert_eq!(ws.active_surface_index(), 0);
}

#[test]
fn test_workspace_new_surface_increments_id() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let id1 = ws.new_surface("T1".to_string()).unwrap();
    let id2 = ws.new_surface("T2".to_string()).unwrap();
    let id3 = ws.new_surface("T3".to_string()).unwrap();
    assert_eq!(id1, SurfaceId(1));
    assert_eq!(id2, SurfaceId(2));
    assert_eq!(id3, SurfaceId(3));
}

#[test]
fn test_workspace_next_surface_id_persists_across_calls() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let id1 = ws.new_surface("T1".to_string()).unwrap();
    let id2 = ws.new_surface("T2".to_string()).unwrap();
    let id3 = ws.new_surface("T3".to_string()).unwrap();
    assert_eq!(id1, SurfaceId(1));
    assert_eq!(id2, SurfaceId(2));
    assert_eq!(id3, SurfaceId(3));
    let surfaces = leaf_surfaces_of(&ws);
    assert_eq!(surfaces[0].id, SurfaceId(1));
    assert_eq!(surfaces[1].id, SurfaceId(2));
    assert_eq!(surfaces[2].id, SurfaceId(3));
}

#[test]
fn test_workspace_next_surface_wraps() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    ws.new_surface("T1".to_string()).unwrap();
    ws.new_surface("T2".to_string()).unwrap();
    ws.new_surface("T3".to_string()).unwrap();
    ws.select_surface(2).unwrap();
    assert_eq!(ws.active_surface_index(), 2);

    // 2 -> 0 (wrap)
    ws.next_surface().unwrap();
    assert_eq!(ws.active_surface_index(), 0);
    // 0 -> 1
    ws.next_surface().unwrap();
    assert_eq!(ws.active_surface_index(), 1);
    // 1 -> 2
    ws.next_surface().unwrap();
    assert_eq!(ws.active_surface_index(), 2);
}

#[test]
fn test_workspace_previous_surface_wraps() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    ws.new_surface("T1".to_string()).unwrap();
    ws.new_surface("T2".to_string()).unwrap();
    ws.new_surface("T3".to_string()).unwrap();
    ws.select_surface(0).unwrap();
    assert_eq!(ws.active_surface_index(), 0);

    // 0 -> 2 (wrap)
    ws.previous_surface().unwrap();
    assert_eq!(ws.active_surface_index(), 2);
    // 2 -> 1
    ws.previous_surface().unwrap();
    assert_eq!(ws.active_surface_index(), 1);
    // 1 -> 0
    ws.previous_surface().unwrap();
    assert_eq!(ws.active_surface_index(), 0);
}

#[test]
fn test_workspace_select_surface_bounds_check() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    ws.new_surface("T1".to_string()).unwrap();
    ws.new_surface("T2".to_string()).unwrap();

    assert_eq!(ws.select_surface(0).unwrap(), ());
    assert_eq!(ws.active_surface_index(), 0);
    assert_eq!(ws.select_surface(1).unwrap(), ());
    assert_eq!(ws.active_surface_index(), 1);

    let result = ws.select_surface(2);
    assert!(matches!(result, Err(WorkspaceError::InvalidSurfaceIndex(2))));

    let result = ws.select_surface(usize::MAX);
    assert!(matches!(result, Err(WorkspaceError::InvalidSurfaceIndex(usize::MAX))));
}

#[test]
fn test_workspace_close_surface_last_errors() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    ws.new_surface("Only".to_string()).unwrap();

    let result = ws.close_surface(0);
    assert!(matches!(result, Err(WorkspaceError::CannotCloseLastSurface)));
    let surfaces = leaf_surfaces_of(&ws);
    assert_eq!(surfaces.len(), 1);
    assert_eq!(surfaces[0].title, "Only");
}

#[test]
fn test_workspace_close_surface_returns_surface() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    ws.new_surface("T1".to_string()).unwrap();
    ws.new_surface("T2".to_string()).unwrap();
    ws.new_surface("T3".to_string()).unwrap();

    let closed = ws.close_surface(1).unwrap();
    assert_eq!(closed.title, "T2");
    assert_eq!(closed.id, SurfaceId(2));

    let surfaces = leaf_surfaces_of(&ws);
    assert_eq!(surfaces.len(), 2);
    assert_eq!(surfaces[0].title, "T1");
    assert_eq!(surfaces[1].title, "T3");
}

#[test]
fn test_workspace_rename_surface_changes_title() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    ws.new_surface("Old".to_string()).unwrap();
    ws.new_surface("Keep".to_string()).unwrap();

    ws.rename_surface(0, "Renamed".to_string()).unwrap();
    let surfaces = leaf_surfaces_of(&ws);
    assert_eq!(surfaces[0].title, "Renamed");
    assert_eq!(surfaces[1].title, "Keep");

    let result = ws.rename_surface(99, "X".to_string());
    assert!(matches!(result, Err(WorkspaceError::InvalidSurfaceIndex(99))));
}

#[test]
fn test_workspace_close_other_surfaces_keeps_active() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    ws.new_surface("T1".to_string()).unwrap();
    ws.new_surface("T2".to_string()).unwrap();
    ws.new_surface("T3".to_string()).unwrap();
    ws.select_surface(1).unwrap();

    let closed = ws.close_other_surfaces().unwrap();
    assert_eq!(closed.len(), 2);
    let mut closed_titles: Vec<&str> = closed.iter().map(|s| s.title.as_str()).collect();
    closed_titles.sort();
    assert_eq!(closed_titles, vec!["T1", "T3"]);

    let surfaces = leaf_surfaces_of(&ws);
    assert_eq!(surfaces.len(), 1);
    assert_eq!(surfaces[0].title, "T2");
    assert_eq!(ws.active_surface_index(), 0);
}

#[test]
fn test_workspace_close_other_surfaces_with_six_surfaces() {
    let mut pane_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    for i in 1..=6 {
        ws.new_surface(format!("T{i}")).unwrap();
    }
    ws.select_surface(2).unwrap();
    assert_eq!(ws.active_surface_index(), 2);

    let closed = ws.close_other_surfaces().unwrap();
    assert_eq!(closed.len(), 5);

    let surfaces = leaf_surfaces_of(&ws);
    assert_eq!(surfaces.len(), 1);
    assert_eq!(surfaces[0].title, "T3");
    assert_eq!(ws.active_surface_index(), 0);
}

#[test]
fn test_set_browser_replaces_leaf() {
    use crate::browser::BrowserPane;

    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let p1 = ws.active_pane;
    let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();

    let mut browser = BrowserPane::new();
    browser.navigate("example.com").unwrap();
    ws.set_browser(p2, browser);

    assert!(ws.root.is_browser_pane(p2));
    assert!(!ws.root.is_browser_pane(p1));
    assert_eq!(ws.pane_count(), 2);

    let b = ws.root.find_browser_mut(p2).expect("browser pane");
    assert_eq!(b.url(), "https://example.com");
}

#[test]
fn test_browser_pane_split_and_close() {
    use crate::browser::BrowserPane;

    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let p1 = ws.active_pane;
    let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();
    ws.set_browser(p2, BrowserPane::new());
    ws.focus_pane(p2);

    // Split the browser pane further — browser nodes are split targets.
    let p3 = ws.split_down(p2, &mut pane_id, &mut split_id).unwrap();
    assert_eq!(ws.pane_count(), 3);
    assert!(ws.root.is_browser_pane(p2));
    assert!(!ws.root.is_browser_pane(p3)); // new leaf is empty terminal slot

    ws.close_pane(p2).unwrap();
    assert_eq!(ws.pane_count(), 2);
    assert!(!ws.root.is_browser_pane(p2));
}

#[test]
fn test_for_each_browser_mut_visits_all() {
    use crate::browser::BrowserPane;

    let mut pane_id = 1;
    let mut split_id = 1;
    let mut ws = make_workspace(1, "Test", &mut pane_id);
    let p1 = ws.active_pane;
    let p2 = ws.split_right(p1, &mut pane_id, &mut split_id).unwrap();
    let p3 = ws.split_down(p1, &mut pane_id, &mut split_id).unwrap();
    ws.set_browser(p2, BrowserPane::new());
    ws.set_browser(p3, BrowserPane::new());

    let mut ids = Vec::new();
    ws.root.for_each_browser_mut(&mut |id, b| {
        ids.push(id);
        b.mark_shown_this_frame();
    });
    ids.sort_by_key(|id| id.0);
    assert_eq!(ids, vec![p2, p3]);
}
