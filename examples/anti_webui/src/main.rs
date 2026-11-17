use anti_webui::prelude::*;
use gds_uim::el;

#[derive(Clone)]
struct Task {
    id: usize,
    title: &'static str,
    priority: bool,
    done: bool,
}

#[derive(Clone, Copy)]
struct Board {
    tasks: Signal<Vec<Task>>,
    priority_only: Signal<bool>,
}

fn initial_tasks() -> Vec<Task> {
    vec![
        Task {
            id: 0,
            title: "Build the view with UIM",
            priority: true,
            done: true,
        },
        Task {
            id: 1,
            title: "Connect AntiWebUI signals",
            priority: true,
            done: false,
        },
        Task {
            id: 2,
            title: "Try a second UI framework",
            priority: false,
            done: false,
        },
    ]
}

impl Board {
    fn new() -> Self {
        Self {
            tasks: signal(initial_tasks()),
            priority_only: signal(false),
        }
    }
}

fn toggle_task(board: Board, id: usize) {
    board.tasks.update(|tasks| {
        if let Some(task) = tasks.iter_mut().find(|task| task.id == id) {
            task.done = !task.done;
        }
    });
}

fn toggle_filter(board: Board) {
    board.priority_only.update(|only| *only = !*only);
}

fn reset(board: Board) {
    batch(|| {
        board.tasks.set(initial_tasks());
        board.priority_only.set(false);
    });
}

fn summary(board: Board) -> String {
    board.tasks.with(|tasks| {
        let completed = tasks.iter().filter(|task| task.done).count();
        format!("{completed} / {} completed", tasks.len())
    })
}

fn task_card(task: Task, board: Board) -> Div {
    let id = task.id;
    el! {
        .div.{
            flex; items_center; gap: 16.0; p: 16.0;
            { rounded: 12.0; border: 1.0; border_color: 0x334155.rgb };
            bg: 0x1e293b.rgb;
            if task->done { bg: 0x172033.rgb };
        }.[
            .div.{ flex; flex_col; flex_1; gap: 6.0 }.[
                task->title.text.{
                    text_size: 16.0;
                    if task->done { text_color: 0x94a3b8.rgb };
                },
                [
                    if task->priority [
                        "PRIORITY".text.{ text_size: 11.0; text_color: 0xfbbf24.rgb },
                    ],
                    (if task->done ^("Completed") else ^("In progress")).text.{
                        text_size: 12.0; text_color: 0x94a3b8.rgb;
                    },
                ],
            ],
            (if task->done ^("Reopen") else ^("Complete")).button.{
                on_click: move (_) => (board, id).toggle_task;
            },
        ]
    }
}

fn task_list(board: Board) -> Div {
    // Read signals inside dyn_view so changing them rebuilds just this subtree.
    let only_priority = board.priority_only.get();
    let tasks: Vec<_> = board
        .tasks
        .get()
        .into_iter()
        .filter(|task| !only_priority || task.priority)
        .collect();
    let empty = tasks.is_empty();
    el! {
        .div.{ flex; flex_col; gap: 12.0 }.[
            if empty ["No tasks match this filter".text],
            ..tasks.{ into_iter; map: task => (task, board).task_card },
        ]
    }
}

fn board_view(board: Board) -> Div {
    el! {
        .div.{
            flex; flex_col; gap: 20.0; p: 28.0; h_full;
            bg: 0x0f172a.rgb; text_color: 0xe2e8f0.rgb;
        }.[
            .div.{ flex; flex_col; gap: 8.0 }.[
                "UIM × AntiWebUI".text.{ text_size: 28.0; font_weight: FontWeight::BOLD },
                "Native views, reactive state, one UIM syntax.".text.{
                    text_size: 14.0; text_color: 0x94a3b8.rgb;
                },
            ],
            .div.{ flex; items_center; gap: 12.0 }.[
                (move () => board.summary).text.{ flex_1; text_size: 16.0 },
                (move () => if board->priority_only.{ get }
                    ^("Show all tasks") else ^("Priority only")
                ).button.{ on_click: move (_) => board.toggle_filter },
                "Reset".button.{ on_click: move (_) => board.reset },
            ],
            (move () => board.task_list).dyn_view,
        ]
    }
}

fn root() -> Div {
    board_view(Board::new())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = std::env::args().nth(1) {
        anti_webui::app::headless::render_png(path, 760, 520, 1.0, root)
    } else {
        App::new().title("UIM × AntiWebUI").size(760, 520).run(root)
    }
}

#[cfg(test)]
mod tests;
