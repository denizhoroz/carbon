//! Drawing a terminal: the grid is copied out under the emulator lock (a
//! snapshot of styled runs, backgrounds, cursor, marks), then painted with
//! the lock released.

use carbon_core::terminal::alacritty_terminal::grid::Dimensions;
use carbon_core::terminal::alacritty_terminal::index::{Column, Line, Point as GridPoint};
use carbon_core::terminal::alacritty_terminal::term::cell::Flags;
use carbon_core::terminal::alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};
use carbon_core::terminal::{CommandMark, Term, Terminal};
use gpui_kit::*;

use super::colors::Palette;
use crate::style::fixed;

/// Cell size and grid placement of the last paint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// Top-left of cell (0, 0).
    pub origin: Point<Pixels>,
    pub cell: Size<Pixels>,
    pub cols: u16,
    pub rows: u16,
}

impl Geometry {
    /// Font metrics and the space available decide cell size and grid size.
    pub fn measure(
        bounds: Bounds<Pixels>,
        font: &Font,
        font_size: Pixels,
        window: &Window,
    ) -> Self {
        let text = window.text_system();
        let font_id = text.resolve_font(font);
        let width = text
            .advance(font_id, font_size, 'm')
            .map(|size| size.width)
            .unwrap_or(font_size * 0.6);
        let height = (font_size * fixed::TERMINAL_LINE_HEIGHT).round();
        let origin = bounds.origin + point(fixed::TERMINAL_GUTTER, fixed::TERMINAL_PADDING_TOP);
        let inner_w = bounds.size.width - fixed::TERMINAL_GUTTER - fixed::TERMINAL_SCROLLBAR_WIDTH;
        let inner_h = bounds.size.height - fixed::TERMINAL_PADDING_TOP;
        let cols = (inner_w / width).floor().max(2.) as u16;
        let rows = (inner_h / height).floor().max(1.) as u16;
        Self {
            origin,
            cell: size(width, height),
            cols,
            rows,
        }
    }

    /// The cell under a window position, clamped to the grid, and whether
    /// the position is in its right half.
    pub fn cell_at(&self, position: Point<Pixels>) -> (usize, usize, bool) {
        let x = ((position.x - self.origin.x) / self.cell.width).max(0.);
        let y = ((position.y - self.origin.y) / self.cell.height).max(0.);
        let col = (x.floor() as usize).min(self.cols.saturating_sub(1) as usize);
        let row = (y.floor() as usize).min(self.rows.saturating_sub(1) as usize);
        (row, col, x.fract() > 0.5)
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Style {
    fg: Hsla,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
}

struct Run {
    col: usize,
    text: String,
    style: Style,
}

/// One visible row, ready to paint.
#[derive(Default)]
struct Row {
    runs: Vec<Run>,
    /// Background rectangles: start column, end column (exclusive), colour.
    backgrounds: Vec<(usize, usize, Hsla)>,
}

pub struct Snapshot {
    rows: Vec<Row>,
    cursor: Option<(usize, usize, CursorShape, String, Hsla)>,
    /// Command marks visible: row and colour.
    marks: Vec<(usize, Hsla)>,
    /// Scrollback: lines above the screen, and how far up the view is.
    history: usize,
    display_offset: usize,
    /// Link under the pointer: row, column range.
    link: Option<(usize, usize, usize)>,
}

impl Snapshot {
    pub fn take(
        term: &Term,
        palette: &Palette,
        marks: &[CommandMark],
        link: Option<(usize, usize, usize)>,
    ) -> Self {
        let content = term.renderable_content();
        let offset = content.display_offset;
        let screen = term.screen_lines();
        let mut rows: Vec<Row> = (0..screen).map(|_| Row::default()).collect();
        let colors = content.colors;
        let selection = content.selection;
        let default_bg = palette.background;

        for indexed in content.display_iter {
            let cell = indexed.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let row_ix = (indexed.point.line.0 + offset as i32) as usize;
            let Some(row) = rows.get_mut(row_ix) else {
                continue;
            };
            let col = indexed.point.column.0;
            let mut fg = palette.resolve(cell.fg, colors);
            let mut bg = match cell.bg {
                Color::Named(NamedColor::Background) => None,
                other => Some(palette.resolve(other, colors)),
            };
            if cell.flags.contains(Flags::INVERSE) {
                let old_bg = bg.unwrap_or(default_bg);
                bg = Some(fg);
                fg = old_bg;
            }
            if cell.flags.contains(Flags::DIM) {
                fg = Palette::dim(fg);
            }
            if selection.is_some_and(|range| range.contains(indexed.point)) {
                bg = Some(palette.selection);
            }
            let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                2
            } else {
                1
            };
            if let Some(bg) = bg {
                match row.backgrounds.last_mut() {
                    Some((_, end, color)) if *end == col && *color == bg => *end = col + width,
                    _ => row.backgrounds.push((col, col + width, bg)),
                }
            }
            let hidden = cell.flags.contains(Flags::HIDDEN);
            let c = if hidden { ' ' } else { cell.c };
            if c == ' '
                && !cell
                    .flags
                    .intersects(Flags::ALL_UNDERLINES | Flags::STRIKEOUT)
            {
                continue;
            }
            let style = Style {
                fg,
                bold: cell.flags.contains(Flags::BOLD),
                italic: cell.flags.contains(Flags::ITALIC),
                underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
                strike: cell.flags.contains(Flags::STRIKEOUT),
            };
            let mut text = c.to_string();
            if let Some(zero_width) = cell.zerowidth() {
                text.extend(zero_width);
            }
            // ASCII cells join runs; anything else is drawn on its own so a
            // fallback font's width cannot shift the columns after it.
            let simple = c.is_ascii() && width == 1 && text.len() == 1;
            match row.runs.last_mut() {
                Some(run)
                    if simple
                        && run.style == style
                        && run.text.is_ascii()
                        && run.col + run.text.len() == col =>
                {
                    run.text.push(c)
                }
                _ => row.runs.push(Run { col, text, style }),
            }
        }

        let cursor = (content.cursor.shape != CursorShape::Hidden && offset == 0)
            .then(|| {
                let point = content.cursor.point;
                let row = (point.line.0 + offset as i32) as usize;
                let cell = &term.grid()[point];
                let text = if cell.c == ' ' {
                    String::new()
                } else {
                    cell.c.to_string()
                };
                (
                    row,
                    point.column.0,
                    content.cursor.shape,
                    text,
                    palette.background,
                )
            })
            .filter(|(row, ..)| *row < screen);

        let history = term.grid().history_size();
        let marks = marks
            .iter()
            .filter_map(|mark| {
                let line = Terminal::grid_line(term, mark.prompt_line)?;
                let row = line.0 + offset as i32;
                let color = match (mark.finished, mark.exit_code) {
                    (true, Some(0)) => palette.success,
                    (true, Some(_)) => palette.error,
                    _ => palette.muted,
                };
                (0..screen as i32)
                    .contains(&row)
                    .then_some((row as usize, color))
            })
            .collect();

        Self {
            rows,
            cursor,
            marks,
            history,
            display_offset: offset,
            link,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        geometry: &Geometry,
        palette: &Palette,
        font: &Font,
        font_size: Pixels,
        focused: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.paint_quad(fill(bounds, palette.background));
        let cell = geometry.cell;
        let at = |row: usize, col: usize| {
            geometry.origin + point(cell.width * col as f32, cell.height * row as f32)
        };

        for (row_ix, row) in self.rows.iter().enumerate() {
            for &(start, end, color) in &row.backgrounds {
                let origin = at(row_ix, start);
                window.paint_quad(fill(
                    Bounds::new(origin, size(cell.width * (end - start) as f32, cell.height)),
                    color,
                ));
            }
        }

        for (row_ix, row) in self.rows.iter().enumerate() {
            for run in &row.runs {
                paint_text(
                    &run.text,
                    run.style,
                    at(row_ix, run.col),
                    font,
                    font_size,
                    cell.height,
                    window,
                    cx,
                );
            }
        }

        if let Some((row, start, end)) = self.link {
            let origin = at(row, start) + point(px(0.), cell.height - px(2.));
            window.paint_quad(fill(
                Bounds::new(origin, size(cell.width * (end - start) as f32, px(1.))),
                palette.foreground,
            ));
        }

        if let Some((row, col, shape, text, text_color)) = &self.cursor {
            let origin = at(*row, *col);
            let block = Bounds::new(origin, cell);
            match (shape, focused) {
                (_, false) => window.paint_quad(outline(block, palette.cursor, BorderStyle::Solid)),
                (CursorShape::Beam, true) => window.paint_quad(fill(
                    Bounds::new(origin, size(px(2.), cell.height)),
                    palette.cursor,
                )),
                (CursorShape::Underline, true) => window.paint_quad(fill(
                    Bounds::new(
                        origin + point(px(0.), cell.height - px(2.)),
                        size(cell.width, px(2.)),
                    ),
                    palette.cursor,
                )),
                (_, true) => {
                    window.paint_quad(fill(block, palette.cursor));
                    if !text.is_empty() {
                        let style = Style {
                            fg: *text_color,
                            bold: false,
                            italic: false,
                            underline: false,
                            strike: false,
                        };
                        paint_text(
                            text,
                            style,
                            origin,
                            font,
                            font_size,
                            cell.height,
                            window,
                            cx,
                        );
                    }
                }
            }
        }

        // Command marks in the gutter, centred on the prompt's row.
        for &(row, color) in &self.marks {
            let mark = fixed::TERMINAL_MARK_SIZE;
            let origin = point(
                bounds.origin.x + (fixed::TERMINAL_GUTTER - mark) / 2.,
                at(row, 0).y + (cell.height - mark) / 2.,
            );
            window.paint_quad(
                fill(Bounds::new(origin, size(mark, mark)), color).corner_radii(mark / 2.),
            );
        }

        // Scrollbar thumb, only with scrollback.
        if self.history > 0 {
            let total = (self.history + self.rows.len()) as f32;
            let track = bounds.size.height;
            let thumb = (track * (self.rows.len() as f32 / total)).max(px(16.));
            let top = (track - thumb)
                * ((self.history - self.display_offset) as f32 / self.history as f32);
            let origin = point(
                bounds.right() - fixed::TERMINAL_SCROLLBAR_WIDTH,
                bounds.origin.y + top,
            );
            window.paint_quad(
                fill(
                    Bounds::new(origin, size(fixed::TERMINAL_SCROLLBAR_WIDTH, thumb)),
                    palette.muted.opacity(0.4),
                )
                .corner_radii(fixed::TERMINAL_SCROLLBAR_WIDTH / 2.),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_text(
    text: &str,
    style: Style,
    origin: Point<Pixels>,
    font: &Font,
    font_size: Pixels,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    let mut font = font.clone();
    if style.bold {
        font.weight = FontWeight::BOLD;
    }
    if style.italic {
        font.style = FontStyle::Italic;
    }
    let run = TextRun {
        len: text.len(),
        font,
        color: style.fg,
        background_color: None,
        underline: style.underline.then(|| UnderlineStyle {
            thickness: px(1.),
            color: Some(style.fg),
            wavy: false,
        }),
        strikethrough: style.strike.then(|| StrikethroughStyle {
            thickness: px(1.),
            color: Some(style.fg),
        }),
    };
    let line = window
        .text_system()
        .shape_line(text.to_string().into(), font_size, &[run], None);
    _ = line.paint(origin, line_height, TextAlign::Left, None, window, cx);
}

/// A viewport row and column as a grid point (the view may be scrolled up).
pub fn grid_point(term: &Term, row: usize, col: usize) -> GridPoint {
    let offset = term.grid().display_offset() as i32;
    GridPoint::new(Line(row as i32 - offset), Column(col))
}
