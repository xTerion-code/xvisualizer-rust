use std::fmt::Write as _;

use crate::color::ColorMode;
use crate::frame::center_x;
use crate::source::{CaptureTarget, PlaybackStream};
use crate::symmetry::LayoutMode;
use crate::theme::Theme;

pub fn render_menu(
    frame: &mut String,
    cols: usize,
    rows: usize,
    selected: usize,
    current: Theme,
    setup: bool,
    use_color: bool,
) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let lines = 9usize;
    let mut top = rows.saturating_sub(lines) / 2;
    for _ in 0..top {
        frame.push_str("\x1b[K\r\n");
    }
    let title = if setup {
        "Setup 1/4 - Select theme"
    } else {
        "Select theme"
    };
    let tx = center_x(cols, title.chars().count());
    frame.push_str(&" ".repeat(tx));
    if use_color {
        frame.push_str("\x1b[1;36m");
    }
    frame.push_str(title);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K\r\n");

    let hint_top = "Up/Down or Left/Right - select  |  Enter - apply";
    let hx = center_x(cols, hint_top.chars().count());
    frame.push_str(&" ".repeat(hx));
    if use_color {
        frame.push_str("\x1b[90m");
    }
    frame.push_str(hint_top);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K\r\n\r\n\x1b[K\r\n");

    for (idx, th) in Theme::all().iter().enumerate() {
        let is_sel = idx == selected;
        let is_cur = *th == current;
        let preview = match th {
            Theme::Classic => "blocks with gaps",
            Theme::Solid => "joined blocks",
        };
        let marker = if is_sel { "> " } else { "  " };
        let cur_tag = if is_cur { "  (current)" } else { "" };
        let line = format!(
            "{marker}{} - {}  [{preview}]{cur_tag}",
            th.name(),
            th.desc()
        );
        let w = line.chars().count();
        let x = center_x(cols, w);
        frame.push_str(&" ".repeat(x));
        if is_sel {
            if use_color {
                frame.push_str("\x1b[7m");
            } else {
                frame.push_str("> ");
            }
        }
        frame.push_str(&line);
        if is_sel && use_color {
            frame.push_str("\x1b[0m");
        }
        frame.push_str("\x1b[K\r\n");
    }

    frame.push_str("\x1b[K\r\n");
    let hint_bot = if setup {
        "1 / 2 - quick select  |  Enter - next  |  Esc - skip setup  |  Q - quit"
    } else {
        "1 / 2 - quick select  |  Esc - back  |  Q - quit"
    };
    let bx = center_x(cols, hint_bot.chars().count());
    frame.push_str(&" ".repeat(bx));
    if use_color {
        frame.push_str("\x1b[90m");
    }
    frame.push_str(hint_bot);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K");
    // Clear any visualizer leftovers below the menu.
    top += lines;
    while top < rows {
        frame.push_str("\r\n\x1b[K");
        top += 1;
    }
}

pub fn render_color_menu(
    frame: &mut String,
    cols: usize,
    rows: usize,
    selected: usize,
    current: ColorMode,
    use_color: bool,
) {
    let modes = ColorMode::all();
    let options: Vec<(String, bool)> = modes
        .iter()
        .map(|m| (format!("{}  [{}]", m.name(), m.desc()), *m == current))
        .collect();
    render_choice(
        frame,
        cols,
        rows,
        &ChoiceMenu {
            title: "Setup 2/4 - Select bar color",
            options: &options,
            selected,
            quick_hint: "1 / 2 / 3 - quick select",
        },
        use_color,
    );
}

pub fn render_layout_menu(
    frame: &mut String,
    cols: usize,
    rows: usize,
    selected: usize,
    current: LayoutMode,
    use_color: bool,
) {
    let modes = LayoutMode::all();
    let options: Vec<(String, bool)> = modes
        .iter()
        .map(|m| (format!("{}  [{}]", m.name(), m.desc()), *m == current))
        .collect();
    render_choice(
        frame,
        cols,
        rows,
        &ChoiceMenu {
            title: "Setup 3/4 - Select layout",
            options: &options,
            selected,
            quick_hint: "1 / 2 - quick select",
        },
        use_color,
    );
}

// Shared option-list screen for the setup wizard steps after the theme menu.
struct ChoiceMenu<'a> {
    title: &'a str,
    options: &'a [(String, bool)],
    selected: usize,
    quick_hint: &'a str,
}

fn render_choice(frame: &mut String, cols: usize, rows: usize, menu: &ChoiceMenu, use_color: bool) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let lines = menu.options.len() + 6;
    let mut top = rows.saturating_sub(lines) / 2;
    for _ in 0..top {
        frame.push_str("\x1b[K\r\n");
    }
    let tx = center_x(cols, menu.title.chars().count());
    frame.push_str(&" ".repeat(tx));
    if use_color {
        frame.push_str("\x1b[1;36m");
    }
    frame.push_str(menu.title);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K\r\n");

    let hint_top = "Up/Down or Left/Right - select  |  Enter - apply";
    let hx = center_x(cols, hint_top.chars().count());
    frame.push_str(&" ".repeat(hx));
    if use_color {
        frame.push_str("\x1b[90m");
    }
    frame.push_str(hint_top);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K\r\n\r\n\x1b[K\r\n");

    for (idx, (label, is_cur)) in menu.options.iter().enumerate() {
        let is_sel = idx == menu.selected;
        let marker = if is_sel { "> " } else { "  " };
        let cur_tag = if *is_cur { "  (current)" } else { "" };
        let line = format!("{marker}{label}{cur_tag}");
        let w = line.chars().count();
        let x = center_x(cols, w);
        frame.push_str(&" ".repeat(x));
        if is_sel {
            if use_color {
                frame.push_str("\x1b[7m");
            } else {
                frame.push_str("> ");
            }
        }
        frame.push_str(&line);
        if is_sel && use_color {
            frame.push_str("\x1b[0m");
        }
        frame.push_str("\x1b[K\r\n");
    }

    frame.push_str("\x1b[K\r\n");
    let hint_bot = format!(
        "{}  |  Enter - next  |  Esc - skip setup  |  Q - quit",
        menu.quick_hint
    );
    let bx = center_x(cols, hint_bot.chars().count());
    frame.push_str(&" ".repeat(bx));
    if use_color {
        frame.push_str("\x1b[90m");
    }
    frame.push_str(&hint_bot);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K");
    // Clear any visualizer leftovers below the menu.
    top += lines;
    while top < rows {
        frame.push_str("\r\n\x1b[K");
        top += 1;
    }
}

pub struct SourceMenu<'a> {
    pub apps: &'a [PlaybackStream],
    pub selected: usize,
    pub target: &'a CaptureTarget,
    pub setup: bool,
}

pub fn render_source_menu(
    frame: &mut String,
    cols: usize,
    rows: usize,
    menu: &SourceMenu,
    use_color: bool,
) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let current_input = match menu.target {
        CaptureTarget::Stream(s) => Some(s.input),
        CaptureTarget::System => None,
    };

    let mut lines: Vec<String> = Vec::with_capacity(menu.apps.len() + 5);
    lines.push(if menu.setup {
        "Setup 4/4 - Select capture source".to_string()
    } else {
        "Select capture source".to_string()
    });
    lines.push("Up/Down - select  |  Enter - capture  |  R - refresh".to_string());
    lines.push(String::new());
    lines.push(menu_row(
        0,
        menu.selected,
        "System",
        "everything you hear",
        matches!(menu.target, CaptureTarget::System),
    ));
    for (i, app) in menu.apps.iter().enumerate() {
        lines.push(menu_row(
            i + 1,
            menu.selected,
            &app.app,
            &app.stream,
            current_input == Some(app.input),
        ));
    }
    if menu.apps.is_empty() {
        lines.push("(no apps playing audio right now)".to_string());
    }
    lines.push(String::new());
    lines.push(if menu.setup {
        "Esc - skip setup  |  Q - quit".to_string()
    } else {
        "Esc - back  |  Q - quit".to_string()
    });

    let mut top = rows.saturating_sub(lines.len()) / 2;
    for _ in 0..top {
        frame.push_str("\x1b[K\r\n");
    }
    for (idx, line) in lines.iter().enumerate() {
        let is_title = idx == 0;
        let is_hint = idx == 1 || idx == lines.len() - 1;
        let is_option = !is_title && !is_hint && !line.is_empty() && !line.starts_with('(');
        if line.is_empty() {
            frame.push_str("\x1b[K\r\n");
            continue;
        }
        let x = center_x(cols, line.chars().count());
        frame.push_str(&" ".repeat(x));
        if is_title && use_color {
            frame.push_str("\x1b[1;36m");
        } else if is_hint && use_color {
            frame.push_str("\x1b[90m");
        } else if is_option && line.starts_with("> ") && use_color {
            frame.push_str("\x1b[7m");
        }
        frame.push_str(line);
        if (is_title || is_hint || (is_option && line.starts_with("> "))) && use_color {
            frame.push_str("\x1b[0m");
        }
        if idx + 1 < lines.len() {
            frame.push_str("\x1b[K\r\n");
        } else {
            frame.push_str("\x1b[K");
        }
    }
    // Clear any visualizer leftovers below the menu.
    top += lines.len();
    while top < rows {
        frame.push_str("\r\n\x1b[K");
        top += 1;
    }
}

fn menu_row(idx: usize, selected: usize, name: &str, detail: &str, is_current: bool) -> String {
    let marker = if idx == selected { "> " } else { "  " };
    let cur = if is_current { "  (current)" } else { "" };
    if detail.is_empty() {
        format!("{marker}{name}{cur}")
    } else {
        format!("{marker}{name} - {detail}{cur}")
    }
}
