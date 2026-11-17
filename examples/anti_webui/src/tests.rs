use super::*;
use anti_webui::core::{
    Ui,
    event::WindowEvent,
    geometry::{Scale, SizeI},
    tree::{NodeKind, with_doc},
};
use std::cell::Cell;

fn texts() -> Vec<String> {
    with_doc(|doc| {
        doc.nodes
            .values()
            .filter_map(|node| match &node.kind {
                NodeKind::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect()
    })
}

fn click(ui: &mut Ui, label: &str, time: u32) {
    let position = with_doc(|doc| {
        let mut id = doc
            .nodes
            .iter()
            .find_map(|(id, node)| {
                matches!(&node.kind, NodeKind::Text(text) if text.text == label).then_some(id)
            })
            .unwrap_or_else(|| panic!("missing button label: {label}"));
        while doc.get(id).unwrap().listeners.click.is_none() {
            id = doc
                .get(id)
                .unwrap()
                .parent
                .expect("label belongs to a button");
        }
        let rect = doc.window_rect(id).expect("button was laid out");
        assert!(rect.width > 0 && rect.height > 0);
        PointF::new(
            (rect.x + rect.width / 2) as f32,
            (rect.y + rect.height / 2) as f32,
        )
    });
    ui.handle_event(WindowEvent::PointerMoved {
        position,
        modifiers: Modifiers::default(),
    });
    for (pressed, time) in [(true, time), (false, time + 1)] {
        ui.handle_event(WindowEvent::PointerButton {
            position,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::default(),
            time,
        });
    }
    ui.update();
}

#[test]
fn native_clicks_update_signals_text_filters_and_conditional_children() {
    let state = Cell::new(None);
    let mut ui = Ui::new(|| {
        let board = Board::new();
        state.set(Some(board));
        board_view(board)
    });
    ui.handle_event(WindowEvent::Resized {
        size: SizeI::new(760, 520),
        scale: Scale::from_f64(1.0),
    });
    ui.handle_event(WindowEvent::Focused(true));
    ui.update();
    let board = state.get().unwrap();
    assert!(texts().contains(&"1 / 3 completed".into()));
    assert_eq!(texts().iter().filter(|text| *text == "PRIORITY").count(), 2);

    click(&mut ui, "Complete", 10);
    assert!(texts().contains(&"2 / 3 completed".into()));
    assert_eq!(board.tasks.get().iter().filter(|task| task.done).count(), 2);

    click(&mut ui, "Priority only", 1000);
    assert!(board.priority_only.get());
    assert!(!texts().contains(&"Try a second UI framework".into()));
    assert!(texts().contains(&"Show all tasks".into()));

    click(&mut ui, "Show all tasks", 2000);
    assert!(texts().contains(&"Try a second UI framework".into()));

    click(&mut ui, "Reset", 3000);
    assert!(texts().contains(&"1 / 3 completed".into()));
    assert!(!board.priority_only.get());

    board.tasks.set(Vec::new());
    ui.update();
    assert!(texts().contains(&"0 / 0 completed".into()));
    assert!(texts().contains(&"No tasks match this filter".into()));
    assert!(!texts().contains(&"Build the view with UIM".into()));
}
