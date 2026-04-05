use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::app::{App, AppMessage, COMMANDS, CommandDef};

pub fn render(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    let sidebar_width = 35;
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(sidebar_width)])
        .split(size);

    let main_area = main_chunks[0];
    let sidebar_area = main_chunks[1];

    let chat_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(main_area);

    let chat_area = chat_chunks[0];
    let input_area = chat_chunks[1];

    render_chat(frame, chat_area, app);
    render_input_section(frame, input_area, app);
    render_sidebar(frame, sidebar_area, app);
}

fn render_chat(frame: &mut Frame, area: Rect, app: &mut App) {
    let chat_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(ratatui::style::Color::Rgb(136, 15, 57)))
        .style(Style::default().bg(ratatui::style::Color::Rgb(18, 18, 24)));

    let inner = chat_block.inner(area);

    if inner.height == 0 || inner.width == 0 {
        frame.render_widget(chat_block, area);
        return;
    }

    let max_width = inner.width as usize;
    let max_lines = inner.height as usize;

    let mut lines: Vec<Line<'static>> = Vec::new();

    for (i, msg) in app.messages.iter().enumerate() {
        // Skip system messages - they're for the LLM, not the user
        if matches!(msg, AppMessage::System(_)) {
            continue;
        }
        if i > 0 {
            lines.push(Line::from(""));
        }
        let msg_lines = format_message(msg, app, max_width);
        lines.extend(msg_lines);
    }

    // Render subagent cards
    for card in &app.subagents {
        lines.push(Line::from(""));
        let is_collapsed = app.collapsed_subagents.contains(&card.id);
        let arrow = if is_collapsed { "▶" } else { "▼" };

        let (status_icon, status_color) = match card.status {
            crate::app::SubAgentStatus::Thinking => ("🤔", ratatui::style::Color::Rgb(244, 140, 6)),
            crate::app::SubAgentStatus::Working => ("⚙️", ratatui::style::Color::Rgb(232, 93, 4)),
            crate::app::SubAgentStatus::Complete => {
                ("✅", ratatui::style::Color::Rgb(80, 104, 238))
            }
            crate::app::SubAgentStatus::Error(_) => ("❌", ratatui::style::Color::Rgb(208, 0, 0)),
        };

        let card_bg = ratatui::style::Color::Rgb(18, 14, 28);

        lines.push(Line::from(vec![
            Span::styled("  ", Style::default().bg(card_bg)),
            Span::styled(
                format!(" {} 🤖 {} ", arrow, card.name),
                Style::default()
                    .fg(ratatui::style::Color::Rgb(255, 186, 8))
                    .add_modifier(Modifier::BOLD)
                    .bg(card_bg),
            ),
            Span::styled(
                format!(" — {}", card.task),
                Style::default()
                    .fg(ratatui::style::Color::DarkGray)
                    .bg(card_bg),
            ),
            Span::styled(
                format!(" [{}]", status_icon),
                Style::default().fg(status_color).bg(card_bg),
            ),
        ]));

        if !is_collapsed && !card.output.is_empty() {
            let max_output = 8;
            let output_lines: Vec<&str> = card.output.lines().take(max_output).collect();
            for line in output_lines {
                lines.push(Line::from(Span::styled(
                    format!("    │ {}", line),
                    Style::default()
                        .fg(ratatui::style::Color::DarkGray)
                        .bg(card_bg),
                )));
            }
            if card.output.lines().count() > max_output {
                lines.push(Line::from(Span::styled(
                    "    │ ...",
                    Style::default().fg(ratatui::style::Color::DarkGray),
                )));
            }
        }
    }

    let wrapped: Vec<Line<'static>> = lines
        .into_iter()
        .flat_map(|line| wrap_line(line, max_width))
        .collect();

    let total = wrapped.len();

    // Use app.scroll_offset if user has scrolled, otherwise auto-scroll to bottom
    let max_scroll = total.saturating_sub(max_lines) as u16;
    let skip = if app.scroll_offset > 0 {
        app.scroll_offset.min(max_scroll)
    } else {
        max_scroll
    };

    let visible: Vec<Line<'static>> = wrapped.into_iter().skip(skip as usize).collect();

    let paragraph = Paragraph::new(Text::from(visible));

    frame.render_widget(chat_block, area);
    frame.render_widget(paragraph, inner);

    if total > max_lines {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None)
            .thumb_symbol("┃");

        let mut scrollbar_state = ScrollbarState::new(total).position(skip as usize);

        frame.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

fn render_input_section(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.file_picker_active {
        render_file_picker(frame, area, app);
        return;
    }

    let has_attachments = !app.attachments.is_empty();
    let prompt_height = if has_attachments { 3 } else { 2 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(prompt_height), Constraint::Length(1)])
        .split(area);

    render_prompt(frame, chunks[0], app);
    render_status(frame, chunks[1], app);
}

fn render_file_picker(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ratatui::style::Color::Rgb(255, 186, 8)))
        .title(" Attach File ");

    let inner = block.inner(area);

    if inner.height == 0 || inner.width == 0 {
        frame.render_widget(block, area);
        return;
    }

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(
            " {} (↑/↓ navigate, Enter select, Esc cancel)",
            app.file_picker_path
        ),
        Style::default().fg(ratatui::style::Color::DarkGray),
    )));

    let max_visible = inner.height.saturating_sub(1) as usize;
    let total = app.file_picker_entries.len();

    if app.file_picker_selected >= app.file_picker_offset + max_visible {
        app.file_picker_offset = app
            .file_picker_selected
            .saturating_sub(max_visible.saturating_sub(1));
    }
    if app.file_picker_selected < app.file_picker_offset {
        app.file_picker_offset = app.file_picker_selected;
    }

    let end = (app.file_picker_offset + max_visible).min(total);

    for i in app.file_picker_offset..end {
        let entry = &app.file_picker_entries[i];
        let is_dir = entry.ends_with('/');
        let is_selected = i == app.file_picker_selected;

        let prefix = if is_selected { "▸ " } else { "  " };
        let icon = if entry == ".." {
            "↰"
        } else if is_dir {
            "📁"
        } else {
            "📄"
        };

        let style = if is_selected {
            Style::default()
                .fg(ratatui::style::Color::Rgb(255, 186, 8))
                .add_modifier(Modifier::BOLD)
        } else if is_dir {
            Style::default().fg(ratatui::style::Color::Rgb(80, 104, 238))
        } else {
            Style::default().fg(ratatui::style::Color::Rgb(168, 180, 246))
        };

        lines.push(Line::from(Span::styled(
            format!("{} {} {}", prefix, icon, entry),
            style,
        )));
    }

    let paragraph = Paragraph::new(Text::from(lines));

    frame.render_widget(block, area);
    frame.render_widget(paragraph, inner);
}

fn render_prompt(frame: &mut Frame, area: Rect, app: &App) {
    let icon = if app.is_processing {
        Span::styled(
            "◆",
            Style::default().fg(ratatui::style::Color::Rgb(232, 93, 4)),
        )
    } else {
        Span::styled(
            "❯",
            Style::default()
                .fg(ratatui::style::Color::Rgb(255, 186, 8))
                .add_modifier(Modifier::BOLD),
        )
    };

    let text_part = if app.is_processing && !app.input_buffer.is_empty() {
        Span::styled(
            app.input_buffer.clone(),
            Style::default().fg(ratatui::style::Color::DarkGray),
        )
    } else if app.input_buffer.is_empty() {
        Span::styled(
            "Ask anything...",
            Style::default().fg(ratatui::style::Color::DarkGray),
        )
    } else {
        Span::styled(
            app.input_buffer.clone(),
            Style::default().fg(ratatui::style::Color::Rgb(250, 163, 7)),
        )
    };

    let line = Line::from(vec![Span::raw("  "), icon, Span::raw(" "), text_part]);

    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(if app.is_processing {
            ratatui::style::Color::Rgb(232, 93, 4)
        } else {
            ratatui::style::Color::Rgb(255, 186, 8)
        }));

    let inner = block.inner(area);

    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(line), inner);

    if !app.attachments.is_empty() && inner.height > 1 {
        let att_text: Vec<Line> = app
            .attachments
            .iter()
            .map(|a| {
                Line::from(Span::styled(
                    format!(
                        "  {} {} ({})",
                        a.kind.icon(),
                        a.path,
                        if a.content.is_some() {
                            "text"
                        } else {
                            "binary"
                        }
                    ),
                    Style::default().fg(ratatui::style::Color::Rgb(80, 104, 238)),
                ))
            })
            .collect();

        let att_rect = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(Text::from(att_text)), att_rect);
    }

    if app.show_commands && !app.is_processing {
        let matches: Vec<&CommandDef> = COMMANDS
            .iter()
            .filter(|c| c.name.starts_with(&app.command_filter))
            .collect();

        if !matches.is_empty() {
            let cmd_text: Vec<Line> = matches
                .iter()
                .map(|c| {
                    let matched = c.name[..app.command_filter.len()].to_string();
                    let rest = &c.name[app.command_filter.len()..];
                    Line::from(vec![
                        Span::styled(
                            format!(" /{}", matched),
                            Style::default()
                                .fg(ratatui::style::Color::Rgb(255, 186, 8))
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            rest.to_string(),
                            Style::default().fg(ratatui::style::Color::Rgb(168, 180, 246)),
                        ),
                        Span::styled(
                            format!(" — {}", c.description),
                            Style::default().fg(ratatui::style::Color::DarkGray),
                        ),
                    ])
                })
                .collect();

            let cmd_height = matches.len() as u16;
            let cmd_area = Rect {
                x: area.x,
                y: area.y.saturating_sub(cmd_height),
                width: area.width,
                height: cmd_height,
            };

            frame.render_widget(
                Paragraph::new(Text::from(cmd_text))
                    .style(Style::default().bg(ratatui::style::Color::Rgb(18, 18, 24))),
                cmd_area,
            );
        }
    }

    if !app.is_processing {
        let cx = area.x + 5 + app.cursor_pos as u16;
        let cy = area.y;
        frame.set_cursor_position((cx, cy));
    }
}

fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let left = if app.is_processing {
        vec![
            Span::styled(
                " ◆ ",
                Style::default().fg(ratatui::style::Color::Rgb(232, 93, 4)),
            ),
            Span::styled(
                "working",
                Style::default()
                    .fg(ratatui::style::Color::Rgb(232, 93, 4))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled("esc", Style::default().fg(ratatui::style::Color::DarkGray)),
            Span::raw(" "),
            Span::styled(
                "interrupt",
                Style::default().fg(ratatui::style::Color::Rgb(80, 104, 238)),
            ),
        ]
    } else {
        vec![
            Span::styled(
                "▣ ",
                Style::default().fg(ratatui::style::Color::Rgb(255, 186, 8)),
            ),
            Span::styled(
                "build",
                Style::default()
                    .fg(ratatui::style::Color::Rgb(255, 186, 8))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                &app.current_model,
                Style::default().fg(ratatui::style::Color::Rgb(168, 180, 246)),
            ),
        ]
    };

    let total_ctx = app.context_breakdown.total();
    let ctx_limit = app.context_limit as usize;
    let pct = if ctx_limit > 0 {
        ((total_ctx as f64 / ctx_limit as f64) * 100.0).min(100.0)
    } else {
        0.0
    };

    let bar_width = 12;
    let filled = (pct / 100.0 * bar_width as f64).round() as usize;
    let empty = bar_width - filled;

    let bar_color = if pct < 50.0 {
        ratatui::style::Color::Rgb(80, 104, 238)
    } else if pct < 80.0 {
        ratatui::style::Color::Rgb(244, 140, 6)
    } else {
        ratatui::style::Color::Rgb(208, 0, 0)
    };

    let bar = format!(
        "{}{} {}%",
        "█".repeat(filled),
        "░".repeat(empty),
        pct.round() as u64
    );

    let right = vec![
        Span::styled(
            " ctx: ",
            Style::default().fg(ratatui::style::Color::DarkGray),
        ),
        Span::styled(bar, Style::default().fg(bar_color)),
        Span::raw("  "),
        Span::styled(
            &app.status_bar.right,
            Style::default().fg(ratatui::style::Color::Rgb(80, 104, 238)),
        ),
    ];

    let mut spans = vec![Span::raw("  ")];
    spans.extend(left);
    spans.push(Span::raw("    "));
    spans.extend(right);

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let sidebar_block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(ratatui::style::Color::Rgb(80, 104, 238)))
        .style(Style::default().bg(ratatui::style::Color::Rgb(14, 14, 20)));

    let inner = sidebar_block.inner(area);

    if inner.height == 0 || inner.width == 0 {
        frame.render_widget(sidebar_block, area);
        return;
    }

    frame.render_widget(sidebar_block, area);

    let mut lines = Vec::new();

    // Session title
    lines.push(Line::from(vec![
        Span::styled(
            " 📁 ",
            Style::default().fg(ratatui::style::Color::Rgb(255, 186, 8)),
        ),
        Span::styled(
            &app.session_title,
            Style::default()
                .fg(ratatui::style::Color::Rgb(255, 186, 8))
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Working directory
    let max_dir_width = inner.width.saturating_sub(4) as usize;
    let dir_display = if app.working_directory.len() > max_dir_width {
        format!(
            "...{}",
            &app.working_directory[app
                .working_directory
                .len()
                .saturating_sub(max_dir_width.saturating_sub(3))..]
        )
    } else {
        app.working_directory.clone()
    };
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(
            dir_display,
            Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
    ]));
    lines.push(Line::from(""));

    // Context stats
    let total_ctx = app.context_breakdown.total();
    let ctx_limit = app.context_limit as usize;
    let pct = if ctx_limit > 0 {
        ((total_ctx as f64 / ctx_limit as f64) * 100.0).min(100.0)
    } else {
        0.0
    };

    let ctx_color = if pct < 50.0 {
        ratatui::style::Color::Rgb(80, 104, 238)
    } else if pct < 80.0 {
        ratatui::style::Color::Rgb(244, 140, 6)
    } else {
        ratatui::style::Color::Rgb(208, 0, 0)
    };

    lines.push(Line::from(vec![Span::styled(
        " Context",
        Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )]));

    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!(
                "instructions  {}",
                app.context_breakdown.system_instructions
            ),
            Style::default().fg(ratatui::style::Color::DarkGray),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("project       {}", app.context_breakdown.project_context),
            Style::default().fg(ratatui::style::Color::DarkGray),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("conversation  {}", app.context_breakdown.conversation),
            Style::default().fg(ratatui::style::Color::DarkGray),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("total         {}/{}", total_ctx, ctx_limit),
            Style::default().fg(ctx_color),
        ),
    ]));

    let bar_width = (inner.width.saturating_sub(4)) as usize;
    let filled = (pct / 100.0 * bar_width as f64).round() as usize;
    let empty = bar_width.saturating_sub(filled);

    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("█".repeat(filled), Style::default().fg(ctx_color)),
        Span::styled(
            "░".repeat(empty),
            Style::default().fg(ratatui::style::Color::DarkGray),
        ),
        Span::styled(
            format!(" {}%", pct.round() as u64),
            Style::default().fg(ratatui::style::Color::DarkGray),
        ),
    ]));
    lines.push(Line::from(""));

    // Tasks
    lines.push(Line::from(vec![Span::styled(
        " Tasks",
        Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )]));

    if app.tasks.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No tasks yet",
            Style::default().fg(ratatui::style::Color::DarkGray),
        )));
    } else {
        for task in &app.tasks {
            let (icon, color) = match task.status {
                crate::app::TaskStatus::Pending => ("○", ratatui::style::Color::DarkGray),
                crate::app::TaskStatus::InProgress => {
                    ("◐", ratatui::style::Color::Rgb(244, 140, 6))
                }
                crate::app::TaskStatus::Complete => ("●", ratatui::style::Color::Rgb(80, 104, 238)),
            };

            let max_title = inner.width.saturating_sub(6) as usize;
            let title = if task.title.len() > max_title {
                format!("{}...", &task.title[..max_title.saturating_sub(3)])
            } else {
                task.title.clone()
            };

            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("[{}] ", icon), Style::default().fg(color)),
                Span::styled(
                    title,
                    Style::default().fg(ratatui::style::Color::Rgb(168, 180, 246)),
                ),
            ]));
        }
    }

    let paragraph = Paragraph::new(Text::from(lines));
    frame.render_widget(paragraph, inner);

    // PR workflow buttons at bottom
    render_pr_buttons(frame, inner, app);
}

fn render_pr_buttons(frame: &mut Frame, inner: Rect, app: &App) {
    let buttons = match &app.pr_state {
        crate::app::PrState::None => vec![],
        crate::app::PrState::HasChanges { branch, changes } => {
            vec![("🔀 Create PR", 0)]
        }
        crate::app::PrState::PrCreated { pr_number, url } => {
            vec![("🔗 Merge PR", 0)]
        }
        crate::app::PrState::MergeReady { pr_number } => {
            vec![("✅ Merge", 0)]
        }
        crate::app::PrState::HasErrors { pr_number, errors } => {
            vec![("🔧 Fix & Commit", 0)]
        }
    };

    if buttons.is_empty() {
        return;
    }

    let btn_height = buttons.len() as u16 + 1;
    let btn_area = Rect {
        x: inner.x,
        y: inner.y.saturating_sub(btn_height),
        width: inner.width,
        height: btn_height,
    };

    for (i, (label, _)) in buttons.iter().enumerate() {
        let is_focused = i == app.focused_button;
        let (bg, fg, border) = if is_focused {
            (
                ratatui::style::Color::Rgb(80, 104, 238),
                ratatui::style::Color::Rgb(255, 255, 255),
                ratatui::style::Color::Rgb(255, 186, 8),
            )
        } else {
            (
                ratatui::style::Color::Rgb(22, 22, 32),
                ratatui::style::Color::Rgb(168, 180, 246),
                ratatui::style::Color::DarkGray,
            )
        };

        let btn_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border))
            .style(Style::default().bg(bg));

        let btn_inner = btn_block.inner(Rect {
            x: btn_area.x,
            y: btn_area.y + i as u16,
            width: btn_area.width,
            height: 1,
        });

        frame.render_widget(
            btn_block.clone(),
            Rect {
                x: btn_area.x,
                y: btn_area.y + i as u16,
                width: btn_area.width,
                height: 1,
            },
        );

        let max_label = btn_inner.width as usize;
        let display_label = if label.len() > max_label {
            format!("{}...", &label[..max_label.saturating_sub(3)])
        } else {
            label.to_string()
        };

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {}", display_label),
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            ))),
            btn_inner,
        );
    }
}

fn format_message(msg: &AppMessage, app: &App, _max_width: usize) -> Vec<Line<'static>> {
    match msg {
        AppMessage::User(text) => {
            let text = text.clone();
            let ts = time::format(&msg.timestamp());
            let mut lines = Vec::new();

            lines.push(Line::from(vec![
                Span::styled(
                    "You",
                    Style::default()
                        .fg(ratatui::style::Color::Rgb(168, 180, 246))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", ts),
                    Style::default().fg(ratatui::style::Color::DarkGray),
                ),
            ]));

            for line in text.lines() {
                lines.push(Line::from(Span::raw(format!("  {}", line))));
            }

            lines
        }

        AppMessage::Assistant(text) => {
            let text = text.clone();
            let ts = time::format(&msg.timestamp());
            let mut lines = Vec::new();

            let working_indicator = if app.is_processing {
                Span::styled(
                    " ● working",
                    Style::default().fg(ratatui::style::Color::Rgb(232, 93, 4)),
                )
            } else {
                Span::raw("")
            };

            lines.push(Line::from(vec![
                Span::styled(
                    "Oni",
                    Style::default()
                        .fg(ratatui::style::Color::Rgb(255, 186, 8))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(
                        " [mode:build] [{}] {}{}",
                        app.current_model, ts, working_indicator
                    ),
                    Style::default().fg(ratatui::style::Color::DarkGray),
                ),
            ]));

            for line in text.lines() {
                lines.push(Line::from(Span::raw(format!("  {}", line))));
            }

            lines
        }

        AppMessage::Thinking(text) => {
            let text = text.clone();
            let mut lines = Vec::new();

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "🧠 Thinking",
                    Style::default()
                        .fg(ratatui::style::Color::Rgb(55, 6, 23))
                        .add_modifier(Modifier::DIM | Modifier::ITALIC),
                ),
            ]));

            for line in text.lines() {
                lines.push(Line::from(Span::styled(
                    format!("    │ {}", line),
                    Style::default()
                        .fg(ratatui::style::Color::Rgb(55, 6, 23))
                        .add_modifier(Modifier::DIM | Modifier::ITALIC),
                )));
            }

            lines
        }

        AppMessage::ToolCall { name, input } => {
            let preview = if input.len() > 80 {
                format!("{}...", &input[..80])
            } else {
                input.clone()
            };

            let block_id = app
                .messages
                .iter()
                .position(|m| {
                    if let AppMessage::ToolCall { name: n, .. } = m {
                        n == name
                    } else {
                        false
                    }
                })
                .unwrap_or(0);

            let is_collapsed = app.collapsed_blocks.contains(&block_id);
            let arrow = if is_collapsed { "▶" } else { "▼" };

            let mut lines = Vec::new();
            lines.push(Line::from(""));

            let bg_style = Style::default().bg(ratatui::style::Color::Rgb(22, 8, 4));

            lines.push(Line::from(vec![
                Span::styled("  ", bg_style),
                Span::styled(
                    format!(" {} 🔧 {}", arrow, name),
                    Style::default()
                        .fg(ratatui::style::Color::Rgb(244, 140, 6))
                        .add_modifier(Modifier::BOLD)
                        .bg(ratatui::style::Color::Rgb(22, 8, 4)),
                ),
                Span::styled(
                    format!(" {}", preview),
                    Style::default()
                        .fg(ratatui::style::Color::DarkGray)
                        .bg(ratatui::style::Color::Rgb(22, 8, 4)),
                ),
            ]));

            if !is_collapsed {
                lines.push(Line::from(Span::styled(
                    format!("    {}", input),
                    Style::default()
                        .fg(ratatui::style::Color::Rgb(106, 4, 15))
                        .bg(ratatui::style::Color::Rgb(22, 8, 4)),
                )));
            }

            lines
        }

        AppMessage::ToolResult { name, is_error } => {
            let (icon, color) = if *is_error {
                ("✗ ", ratatui::style::Color::Rgb(208, 0, 0))
            } else {
                ("✓ ", ratatui::style::Color::Rgb(80, 104, 238))
            };

            let bg_style = Style::default().bg(ratatui::style::Color::Rgb(22, 8, 4));

            vec![
                Line::from(Span::styled("  ", bg_style)),
                Line::from(vec![
                    Span::styled("  ", bg_style),
                    Span::styled(
                        format!("{} {}", icon, name),
                        Style::default()
                            .fg(color)
                            .bg(ratatui::style::Color::Rgb(22, 8, 4)),
                    ),
                ]),
            ]
        }

        AppMessage::Error(text) => {
            let text = text.clone();
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        "✗ ",
                        Style::default().fg(ratatui::style::Color::Rgb(208, 0, 0)),
                    ),
                    Span::styled(
                        text,
                        Style::default()
                            .fg(ratatui::style::Color::Rgb(220, 47, 2))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
            ]
        }

        AppMessage::System(text) => {
            let text = text.clone();
            vec![Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    text,
                    Style::default()
                        .fg(ratatui::style::Color::Rgb(80, 104, 238))
                        .add_modifier(Modifier::DIM),
                ),
            ])]
        }
    }
}

fn wrap_line(line: Line<'static>, max_width: usize) -> Vec<Line<'static>> {
    if max_width == 0 {
        return vec![line];
    }

    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

    if text.chars().count() <= max_width {
        return vec![line];
    }

    let mut result = Vec::new();
    let chars: Vec<char> = text.chars().collect();

    let mut span_idx = 0;
    let mut char_in_span = 0;

    for chunk_start in (0..chars.len()).step_by(max_width) {
        let chunk_end = (chunk_start + max_width).min(chars.len());
        let chunk: String = chars[chunk_start..chunk_end].iter().collect();

        let mut spans = Vec::new();
        let mut remaining = chunk.clone();

        while !remaining.is_empty() {
            while span_idx < line.spans.len() {
                let span = &line.spans[span_idx];
                let span_chars: Vec<char> = span.content.chars().collect();
                let available = span_chars.len().saturating_sub(char_in_span);

                if available == 0 {
                    span_idx += 1;
                    char_in_span = 0;
                    continue;
                }

                let take = available.min(remaining.chars().count());
                let span_text: String = span_chars[char_in_span..char_in_span + take]
                    .iter()
                    .collect();

                if span_text.is_empty() {
                    span_idx += 1;
                    char_in_span = 0;
                    continue;
                }

                spans.push(ratatui::text::Span::styled(span_text, span.style));

                char_in_span += take;
                remaining = remaining.chars().skip(take).collect();

                if char_in_span >= span_chars.len() {
                    span_idx += 1;
                    char_in_span = 0;
                }

                break;
            }

            if remaining.is_empty() {
                break;
            }
        }

        if !spans.is_empty() {
            result.push(Line::from(spans));
        }
    }

    if result.is_empty() {
        result.push(line);
    }

    result
}

mod time {
    pub fn format(dt: &chrono::DateTime<chrono::Local>) -> String {
        format!("[{}]", dt.format("%H:%M"))
    }
}
