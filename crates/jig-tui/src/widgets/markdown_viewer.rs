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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let lines = markdown_to_lines("");
        assert!(lines.is_empty(), "empty markdown must produce empty output");
    }

    #[test]
    fn test_heading_produces_styled_line() {
        let lines = markdown_to_lines("# Hello");
        assert!(!lines.is_empty(), "heading must produce at least one line");
        // First line should have Cyan + Bold style (H1)
        let first = &lines[0];
        let span = &first.spans[0];
        assert_eq!(span.style.fg, Some(Color::Cyan));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
        assert!(span.content.contains("Hello"));
    }

    #[test]
    fn test_h2_heading_is_blue() {
        let lines = markdown_to_lines("## Sub heading");
        let first = &lines[0];
        let span = &first.spans[0];
        assert_eq!(span.style.fg, Some(Color::Blue));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_bold_text() {
        let lines = markdown_to_lines("**bold text**");
        assert!(!lines.is_empty());
        let span = &lines[0].spans[0];
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
        assert!(span.content.contains("bold text"));
    }

    #[test]
    fn test_code_block_yellow() {
        let lines = markdown_to_lines("```\ncode here\n```");
        // Find the line with actual code content
        let code_line = lines.iter().find(|l| {
            l.spans.iter().any(|s| s.content.contains("code here"))
        });
        assert!(code_line.is_some(), "must contain a line with 'code here'");
        let span = code_line.unwrap().spans.iter().find(|s| s.content.contains("code here")).unwrap();
        assert_eq!(span.style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_multiple_paragraphs_have_separator() {
        let lines = markdown_to_lines("para one\n\npara two");
        // Should have at least 3 lines: "para one", "", "para two"
        assert!(lines.len() >= 3, "two paragraphs must produce at least 3 lines (with blank separator)");
        // Check there's an empty separator line
        let has_empty = lines.iter().any(|l| l.spans.is_empty() || (l.spans.len() == 1 && l.spans[0].content.is_empty()));
        assert!(has_empty, "must have an empty separator line between paragraphs");
    }

    #[test]
    fn test_inline_code_yellow() {
        let lines = markdown_to_lines("Use `foo` here");
        let span = lines[0].spans.iter().find(|s| s.content.contains("foo")).unwrap();
        assert_eq!(span.style.fg, Some(Color::Yellow));
    }
}
