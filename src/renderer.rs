use crate::{
    game::{GameInput, RusdleState, RES_CORRECT, RES_DEFAULT, RES_PRESENT, RES_WRONG},
    rendering::{render_boxed_word, render_message_centered},
};
use crossterm::{
    cursor::{self, MoveDown, MoveLeft, MoveTo, MoveToColumn},
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Color, ContentStyle, PrintStyledContent, ResetColor, Stylize},
    terminal::{self, disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::{io, iter::repeat};

pub(crate) trait Renderer {
    fn next_input(&self) -> io::Result<Option<GameInput>>;
    fn render(&mut self, state: &RusdleState) -> io::Result<()>;
}

pub(crate) fn with_terminal<F: FnOnce(&mut dyn Renderer) -> io::Result<()>>(
    func: F,
) -> io::Result<()> {
    let stdout = Box::leak(Box::new(io::stdout()));
    let mut r = TerminalRenderer {
        stdout: stdout.lock(),
    };
    r.init()?;
    func(&mut r)?;
    r.destroy()
}

struct TerminalRenderer {
    stdout: std::io::StdoutLock<'static>,
}

impl TerminalRenderer {
    fn init(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        execute!(self.stdout, cursor::Hide)
    }
    fn destroy(&mut self) -> io::Result<()> {
        disable_raw_mode()?;
        execute!(
            self.stdout,
            ResetColor,
            Clear(ClearType::CurrentLine),
            MoveToColumn(0),
            cursor::Show
        )
    }
}

impl TerminalRenderer {
    fn render_header(&mut self) -> io::Result<()> {
        render_boxed_word(
            &mut self.stdout,
            "RUSDLE",
            repeat(ContentStyle::new().blue().bold().italic()),
        )?;
        render_message_centered(&mut self.stdout, "Wordle in Rust".bold())?;
        queue!(self.stdout, MoveDown(1))
    }

    fn render_guesses(&mut self, state: &RusdleState) -> io::Result<()> {
        let mut render_guess = |guess: &str, result: &[u8; 5]| {
            render_boxed_word(
                &mut self.stdout,
                &guess,
                result.iter().map(|r| result_colours(*r)),
            )
        };
        for (guess, result) in state.guesses.iter() {
            render_guess(guess, result)?;
        }
        if !state.is_over() {
            render_guess(&format!("{}_    ", state.entry)[..5], &[RES_DEFAULT; 5])?;
            for _ in state.guesses.len()..5 {
                render_guess("     ", &[RES_DEFAULT; 5])?;
            }
        } else {
            for _ in state.guesses.len()..6 {
                render_guess("     ", &[RES_DEFAULT; 5])?;
            }
        }
        Ok(())
    }

    fn render_keyboard(&mut self, state: &RusdleState) -> io::Result<()> {
        let (cols, _) = terminal::size()?;
        let mut render_keyrow = |row: &str| -> io::Result<()> {
            queue!(
                &mut self.stdout,
                MoveToColumn(cols / 2 - row.len() as u16),
                MoveDown(1)
            )?;
            let mut prev_style = ContentStyle::new();
            for c in row.chars() {
                if c == ' ' {
                    continue;
                }
                let style = match state.clues.get(&c).map(|r| *r) {
                    Some(RES_WRONG) => ContentStyle::new()
                        .dark_grey()
                        .on(Color::from((32, 32, 32))),
                    Some(r) => result_colours(r),
                    None => ContentStyle::new().black().on_dark_grey(),
                };

                queue!(
                    self.stdout,
                    PrintStyledContent(
                        prev_style
                            .clone()
                            .with(style.background_color.unwrap_or(Color::White))
                            .apply('▐')
                    ),
                    PrintStyledContent(style.apply(c)),
                    PrintStyledContent(style.clone().black().apply('▐')),
                    MoveLeft(1),
                )?;
                prev_style = style;
            }
            Ok(())
        };

        render_keyrow("QWERTYUIOP")?;
        render_keyrow("ASDFGHJKL")?;
        render_keyrow("ZXCVBNM ")?;
        queue!(self.stdout, MoveDown(2))
    }

    fn render_message(&mut self, state: &RusdleState) -> Result<(), io::Error> {
        let message = if state.is_over() {
            if state.is_win() {
                "Winner!".green()
            } else {
                "Loser!".red()
            }
        } else {
            match &state.last_error {
                Some(msg) => msg.as_str().dark_yellow(),
                _ => "".stylize(),
            }
        };

        render_message_centered(&mut self.stdout, message.slow_blink())
    }
}

impl Renderer for TerminalRenderer {
    fn next_input(&self) -> io::Result<Option<GameInput>> {
        Ok(match read()? {
            Event::Key(event) => match event {
                KeyEvent {
                    modifiers: KeyModifiers::CONTROL,
                    code: KeyCode::Char('c'),
                } => Some(GameInput::Quit),
                KeyEvent {
                    modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                    code,
                } => match code {
                    KeyCode::Char(c) if c.is_ascii_alphabetic() => {
                        Some(GameInput::Input(c.to_ascii_uppercase()))
                    }
                    KeyCode::Backspace => Some(GameInput::Delete),
                    KeyCode::Enter => Some(GameInput::Submit),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
    }

    fn render(&mut self, state: &RusdleState) -> io::Result<()> {
        queue!(self.stdout, Clear(ClearType::All), ResetColor, MoveTo(0, 0))?;
        self.render_header()?;
        self.render_guesses(&state)?;
        self.render_keyboard(&state)?;
        self.render_message(&state)?;
        execute!(self.stdout, MoveDown(1), MoveToColumn(0))
    }
}

fn result_colours(r: u8) -> ContentStyle {
    match r {
        RES_DEFAULT => ContentStyle::new()
            .white()
            .on(Color::from((32, 32, 32)))
            .bold(),
        RES_WRONG => ContentStyle::new().black().on_dark_grey(),
        RES_PRESENT => ContentStyle::new().black().on_dark_yellow().bold(),
        RES_CORRECT => ContentStyle::new().black().on_dark_green().bold(),
        _ => panic!("unknown char result {}", r),
    }
}
