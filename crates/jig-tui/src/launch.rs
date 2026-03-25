/// Launch transition screen — shown during assembly before handing off to Claude Code.
///
/// Decision (brainstorm §7): Brief assembly status screen before handing off to Claude Code.
/// - Minimum display: 500ms
/// - Each step transitions from ⟳ to ✓/✗
/// - Stays up on failure for readability
use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::theme::active_theme;

const MIN_DISPLAY_MS: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
pub struct AssemblyStep {
    pub label: String,
    pub status: StepStatus,
    pub detail: Option<String>,
    pub elapsed_ms: Option<u64>,
}

impl AssemblyStep {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            status: StepStatus::Pending,
            detail: None,
            elapsed_ms: None,
        }
    }
}

pub struct LaunchScreen {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    steps: Vec<AssemblyStep>,
    start: Instant,
}

impl LaunchScreen {
    pub fn new(step_labels: &[&str]) -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let steps = step_labels.iter().map(|&l| AssemblyStep::new(l)).collect();

        Ok(Self {
            terminal,
            steps,
            start: Instant::now(),
        })
    }

    pub fn set_step_running(&mut self, idx: usize) {
        if let Some(step) = self.steps.get_mut(idx) {
            step.status = StepStatus::Running;
        }
        let _ = self.draw();
    }

    pub fn set_step_done(&mut self, idx: usize, detail: Option<String>) {
        if let Some(step) = self.steps.get_mut(idx) {
            step.status = StepStatus::Done;
            step.detail = detail;
            step.elapsed_ms = Some(self.start.elapsed().as_millis() as u64);
        }
        let _ = self.draw();
    }

    pub fn set_step_failed(&mut self, idx: usize, detail: Option<String>) {
        if let Some(step) = self.steps.get_mut(idx) {
            step.status = StepStatus::Failed;
            step.detail = detail;
            step.elapsed_ms = Some(self.start.elapsed().as_millis() as u64);
        }
        let _ = self.draw();
        // Stay up for readability after failure
        std::thread::sleep(Duration::from_secs(2));
    }

    fn draw(&mut self) -> io::Result<()> {
        let steps = self.steps.clone();
        let theme = active_theme();

        self.terminal.draw(|frame| {
            let area = frame.area();
            let block = Block::default()
                .title(" jig — Launching session... ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_focused));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let mut lines: Vec<Line<'static>> = Vec::with_capacity(steps.len() + 2);
            lines.push(Line::from(""));

            for step in &steps {
                let (icon, icon_style) = match step.status {
                    StepStatus::Pending => (" ", Style::default().fg(Color::DarkGray)),
                    StepStatus::Running => ("⟳", Style::default().fg(Color::Yellow)),
                    StepStatus::Done => ("✓", Style::default().fg(Color::Green)),
                    StepStatus::Failed => ("✗", Style::default().fg(Color::Red)),
                };
                let mut spans = vec![
                    Span::styled(format!("  {icon} "), icon_style),
                    Span::raw(step.label.clone()),
                ];
                if let Some(detail) = &step.detail {
                    spans.push(Span::styled(
                        format!(" — {detail}"),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                if let Some(ms) = step.elapsed_ms {
                    if step.status == StepStatus::Failed {
                        spans.push(Span::styled(
                            format!(" ({ms}ms)"),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }
                lines.push(Line::from(spans));
            }

            let para = Paragraph::new(lines);
            frame.render_widget(para, inner);
        })?;
        Ok(())
    }

    /// Ensures minimum display time then restores terminal.
    pub fn finish_and_restore(self) {
        let elapsed = self.start.elapsed();
        let min_display = Duration::from_millis(MIN_DISPLAY_MS);
        if elapsed < min_display {
            std::thread::sleep(min_display - elapsed);
        }
        // Terminal restored via Drop
    }
}

/// Restores terminal state. MUST be called before execv("claude").
pub fn restore_terminal() {
    let _ = execute!(
        io::stdout(),
        DisableMouseCapture,       // 1st
        LeaveAlternateScreen,      // 2nd
        cursor::Show,              // 3rd
    );
    let _ = disable_raw_mode();    // 4th
}

impl Drop for LaunchScreen {
    fn drop(&mut self) {
        restore_terminal();
    }
}
