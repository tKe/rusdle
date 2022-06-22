use clap::Parser;
use crossterm::event::{read, Event, KeyCode, KeyEvent, KeyModifiers};
use game::{GameInput, GameMode, RusdleState, WordSet};
use std::io;
use std::path::PathBuf;

mod game;
mod renderer;
mod rendering;

#[derive(Parser)]
struct Cli {
    #[clap(arg_enum, default_value_t = game::GameMode::Wordle)]
    mode: GameMode,
    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    word_list: Option<PathBuf>,
    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    dictionary: Option<PathBuf>,
}

fn main() -> Result<(), io::Error> {
    let cli = Cli::parse();
    let mut game = RusdleState::new(WordSet::load(cli.word_list, cli.dictionary)?, cli.mode);

    renderer::Renderer::with(|r| {
        loop {
            r.render(&game)?;
            if game.is_over() {
                break;
            }
            match read()? {
                Event::Key(event) => match event {
                    KeyEvent {
                        modifiers: KeyModifiers::CONTROL,
                        code: KeyCode::Char('c'),
                    } => break,
                    KeyEvent {
                        modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                        code,
                    } => {
                        if let Some(input) = match code {
                            KeyCode::Char(c) if c.is_ascii_alphabetic() => {
                                Some(GameInput::Input(c.to_ascii_uppercase()))
                            }
                            KeyCode::Backspace => Some(GameInput::Delete),
                            KeyCode::Enter => Some(GameInput::Submit),
                            _ => None,
                        } {
                            game.handle_input(input)
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(())
    })
}

trait MutVecExt<T> {
    fn remove_item(&mut self, val: T) -> bool;
}

impl<T: PartialEq> MutVecExt<T> for Vec<T> {
    fn remove_item(&mut self, val: T) -> bool {
        if let Some(idx) = self.iter().position(|x| *x == val) {
            self.swap_remove(idx);
            true
        } else {
            false
        }
    }
}
