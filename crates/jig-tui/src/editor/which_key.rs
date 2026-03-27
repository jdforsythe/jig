use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn editor_bindings() -> Vec<Line<'static>> {
    let key_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let spacer = Span::raw("   ");

    vec![
        Line::from(vec![
            Span::styled("J/Tab", key_style),
            Span::raw(" "),
            Span::styled("next section", desc_style),
            spacer.clone(),
            Span::styled("K/S-Tab", key_style),
            Span::raw(" "),
            Span::styled("prev section", desc_style),
        ]),
        Line::from(vec![
            Span::styled("j/k", key_style),
            Span::raw(" "),
            Span::styled("move in list", desc_style),
            spacer.clone(),
            Span::styled("gg/G", key_style),
            Span::raw(" "),
            Span::styled("first/last section", desc_style),
        ]),
        Line::from(vec![
            Span::styled("a", key_style),
            Span::raw(" "),
            Span::styled("add item", desc_style),
            spacer.clone(),
            Span::styled("d", key_style),
            Span::raw(" "),
            Span::styled("delete item", desc_style),
            spacer.clone(),
            Span::styled("i", key_style),
            Span::raw(" "),
            Span::styled("edit item", desc_style),
        ]),
        Line::from(vec![
            Span::styled("Ctrl-Z", key_style),
            Span::raw(" "),
            Span::styled("undo", desc_style),
            spacer.clone(),
            Span::styled("Ctrl-S/:w", key_style),
            Span::raw(" "),
            Span::styled("save", desc_style),
        ]),
        Line::from(vec![
            Span::styled("?", key_style),
            Span::raw(" "),
            Span::styled("which-key", desc_style),
            spacer.clone(),
            Span::styled("q/Esc", key_style),
            Span::raw(" "),
            Span::styled("exit", desc_style),
        ]),
    ]
}
