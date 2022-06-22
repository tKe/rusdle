use clap::Parser;
use game::{GameMode, RusdleState, WordSet};
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

    renderer::with_terminal(|r| {
        loop {
            r.render(&game)?;
            if game.is_over() {
                break;
            }
            if let Some(input) = r.next_input()? {
                game.handle_input(input)
            }
        }
        Ok(())
    })
}
