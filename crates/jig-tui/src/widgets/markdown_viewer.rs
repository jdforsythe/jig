use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Converts Markdown text to a `Vec<Line<'static>>` for rendering in a Paragraph widget.
/// Uses `Vec::with_capacity(256)` to avoid repeated allocations.
pub fn markdown_to_lines(markdown: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(256);
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];

    let parser = Parser::new(markdown);

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let style = match level {
                    HeadingLevel::H1 => Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                    HeadingLevel::H2 => Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                    _ => Style::default().add_modifier(Modifier::BOLD),
                };
                style_stack.push(style);
            }
            Event::End(TagEnd::Heading(_)) => {
                style_stack.pop();
                if !current_spans.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
                lines.push(Line::from(""));
            }
            Event::Start(Tag::Strong) => {
                let parent = *style_stack.last().unwrap_or(&Style::default());
                style_stack.push(parent.add_modifier(Modifier::BOLD));
            }
            Event::End(TagEnd::Strong) => {
                style_stack.pop();
            }
            Event::Start(Tag::Emphasis) => {
                let parent = *style_stack.last().unwrap_or(&Style::default());
                style_stack.push(parent.add_modifier(Modifier::ITALIC));
            }
            Event::End(TagEnd::Emphasis) => {
                style_stack.pop();
            }
            Event::Start(Tag::CodeBlock(_)) => {
                style_stack.push(Style::default().fg(Color::Yellow));
            }
            Event::End(TagEnd::CodeBlock) => {
                style_stack.pop();
                lines.push(Line::from(""));
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                if !current_spans.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
                lines.push(Line::from(""));
            }
            Event::Text(text) => {
                let style = *style_stack.last().unwrap_or(&Style::default());
                // to_string() converts CowStr to String, satisfying 'static lifetime
                current_spans.push(Span::styled(text.to_string(), style));
            }
            Event::Code(code) => {
                let style = Style::default().fg(Color::Yellow);
                current_spans.push(Span::styled(
                    format!("`{code}`"),
                    style,
                ));
            }
            Event::SoftBreak | Event::HardBreak => {
                if !current_spans.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
            }
            _ => {}
        }
    }

    // Flush any remaining spans
    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }

    lines
}
