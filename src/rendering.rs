use crossterm::{
    cursor::{MoveDown, MoveLeft, MoveToColumn, MoveUp},
    queue,
    style::{ContentStyle, PrintStyledContent, StyledContent, Stylize},
    terminal,
};
use std::io::{self, Write};

pub(crate) fn render_boxed_word<T: Write, I>(mut stdout: T, word: &str, styles: I) -> io::Result<()>
where
    I: Iterator<Item = ContentStyle>,
{
    let x = (terminal::size()?.0 / 2) - (4 * word.len() as u16 - 1) / 2;
    for ((ci, c), style) in word.char_indices().zip(styles) {
        let s = &mut stdout;
        let cx = x + (ci as u16 * 4);
        queue!(s, MoveToColumn(cx))?;
        draw_charbox(s, c, style)?;
    }
    queue!(stdout, MoveDown(3))
}

pub(crate) fn render_message_centered<T: Write>(
    mut stdout: T,
    message: StyledContent<&str>,
) -> io::Result<()> {
    queue!(
        stdout,
        MoveToColumn(terminal::size()?.0 / 2 - message.content().len() as u16 / 2),
        PrintStyledContent(message)
    )
}

fn draw_charbox<T: Write>(mut stdout: T, c: char, style: ContentStyle) -> io::Result<()> {
    match style {
        ContentStyle {
            background_color: None,
            ..
        } => queue!(
            stdout,
            PrintStyledContent(style.apply("\u{250c}\u{2500}\u{2510}")),
            MoveDown(1),
            MoveLeft(3),
            PrintStyledContent(style.apply("\u{2502} \u{2502}")),
            MoveDown(1),
            MoveLeft(3),
            PrintStyledContent(style.apply("\u{2514}\u{2500}\u{2518}")),
            MoveUp(1),
            MoveLeft(2),
            PrintStyledContent(style.apply(c)),
            MoveUp(1),
            MoveLeft(1),
        ),
        _ => queue!(
            stdout,
            PrintStyledContent(style.black().negative().apply("\u{2584}\u{2584}\u{2584}")),
            MoveDown(1),
            MoveLeft(3),
            PrintStyledContent(style.apply(format!(" {} ", c))),
            MoveDown(1),
            MoveLeft(3),
            PrintStyledContent(style.black().negative().apply("\u{2580}\u{2580}\u{2580}")),
            MoveUp(2),
            MoveLeft(2),
        ),
    }
}
