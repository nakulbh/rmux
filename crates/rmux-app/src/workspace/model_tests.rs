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
