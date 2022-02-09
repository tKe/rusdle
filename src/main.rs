use std::{
    io::{
        self,
        prelude::*,
        BufReader,
        Stdout,
    },
    fs::File,
    path::Path,
    fmt::{Debug, Formatter},
    iter::repeat,
};
use rand::seq::SliceRandom;
use clap::{Parser, ArgEnum};
use std::collections::HashMap;
use std::path::PathBuf;
use chrono::{DateTime, Local, TimeZone};
use crossterm::{
    cursor::{
        self,
        MoveTo,
        MoveUp,
        MoveDown,
        MoveLeft,
        MoveToColumn,
    },
    event::{
        read,
        Event,
        KeyCode,
        KeyEvent,
        KeyModifiers,
    },
    execute,
    queue,
    style::{
        Color,
        PrintStyledContent,
        ResetColor,
        Stylize,
        ContentStyle,
        StyledContent,
    },
    terminal::{
        self,
        Clear,
        ClearType,
        disable_raw_mode,
        enable_raw_mode,
    },
};

fn lines_from_file(filename: impl AsRef<Path>) -> io::Result<Vec<String>> {
    let file = File::open(filename)?;
    BufReader::new(file).lines().collect()
}

#[derive(Parser)]
struct Cli {
    #[clap(arg_enum, default_value_t = GameMode::Wordle)]
    mode: GameMode,
    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    word_list: Option<PathBuf>,
    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    dictionary: Option<PathBuf>,
}

fn main() -> Result<(), io::Error> {
    let cli = Cli::parse();
    let mut game = RusdleState::new(
        WordSet::load(cli.word_list, cli.dictionary)?,
        cli.mode,
    );

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, cursor::Hide)?;
    loop {
        game.render(&mut stdout)?;
        if game.is_over() {
            break;
        }
        match read()? {
            Event::Key(event) => match event {
                KeyEvent {
                    modifiers: KeyModifiers::CONTROL, code: KeyCode::Char('c')
                } => break,
                KeyEvent {
                    modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                    code
                } => game.handle_key(code),
                _ => {}
            },
            _ => {}
        }
    }

    disable_raw_mode()?;
    execute!(stdout, ResetColor,
        Clear(ClearType::CurrentLine),
        MoveToColumn(0),
        cursor::Show)
}

#[derive(Clone)]
struct WordSet {
    wordlist: Vec<String>,
    valid_guesses: Vec<String>,
}

impl Debug for WordSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("WordSet")
    }
}

impl WordSet {
    fn load(wordlist_path: Option<impl AsRef<Path>>, guesses_path: Option<impl AsRef<Path>>) -> io::Result<Self> {
        let wordlist = wordlist_path
            .map(|p| lines_from_file(p))
            .unwrap_or(Ok(include_str!("../data/wordlist.txt").lines().map(|s| String::from(s)).collect()))?;
        let valid_guesses = guesses_path
            .map(|p| lines_from_file(p))
            .unwrap_or(Ok(include_str!("../data/guesses.txt").lines().map(|s| String::from(s)).collect()))?;

        Ok(WordSet { wordlist, valid_guesses })
    }

    fn is_valid(&self, word: &str) -> bool {
        let needle = &word.to_lowercase();
        self.wordlist.contains(needle) || self.valid_guesses.contains(needle)
    }

    fn word_of_the_day(&self) -> String {
        let epoch: DateTime<Local> = Local.ymd(2021, 6, 19).and_hms(0, 0, 0);
        let idx = (Local::now().date().and_hms(0, 0, 0).timestamp() - epoch.timestamp()) / 86400;
        self.wordlist.get(idx as usize % self.wordlist.len())
            .unwrap().to_ascii_uppercase()
    }

    fn random_word(&self) -> String {
        self.wordlist.choose(&mut rand::thread_rng())
            .unwrap().to_ascii_uppercase()
    }
}

#[derive(Debug)]
struct RusdleState {
    words: WordSet,
    target: Vec<char>,
    entry: String,
    last_error: Option<String>,
    guesses: Vec<(String, [u8; 5])>,
    clues: HashMap<char, u8>,
}

#[derive(ArgEnum, Clone)]
enum GameMode {
    Wordle,
    RandomWord,
}

impl RusdleState {
    pub fn new(words: WordSet, mode: GameMode) -> Self {
        let word = match mode {
            GameMode::RandomWord => words.random_word(),
            GameMode::Wordle => words.word_of_the_day()
        };
        Self::new_with_target(words, &word)
    }

    fn new_with_target(words: WordSet, target: &str) -> RusdleState {
        assert!(words.is_valid(target));
        let target = target.chars().collect();
        Self {
            words,
            target,
            last_error: None,
            entry: String::with_capacity(5),
            guesses: Vec::with_capacity(6),
            clues: HashMap::with_capacity(26),
        }
    }
}

impl RusdleState {
    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) if c.is_ascii_alphabetic() => if self.entry.len() < 5 {
                self.entry.push(c.to_ascii_uppercase())
            },
            KeyCode::Backspace => if self.entry.len() > 0 {
                self.entry.pop();
            },
            KeyCode::Enter => if self.entry.len() == 5 {
                self.process_guess();
            }
            _ => {}
        }
    }

    fn is_over(&self) -> bool {
        self.guesses.len() == 6 || self.is_win()
    }

    fn is_win(&self) -> bool {
        match self.guesses.last() {
            Some((_, r)) if *r == [RES_CORRECT; 5] => true,
            _ => false,
        }
    }

    fn process_guess(&mut self) {
        if !(self.words.is_valid(&self.entry)) {
            self.last_error = Some(format!("Word '{}' is not valid.", self.entry))
        } else {
            self.last_error = None;
            let guess = self.entry.clone();
            let result = self.compare_guess(&guess);

            guess.chars().zip(result)
                .for_each(|(c, r)| {
                    let clue = self.clues.entry(c).or_insert(r);
                    if r > *clue { *clue = r }
                });

            self.guesses.push((guess, result));
            self.entry.clear()
        }
    }

    fn compare_guess(&mut self, guess: &str) -> [u8; 5] {
        let mut unmatched: Vec<char> = self.target.iter().cloned()
            .zip(guess.chars())
            .filter_map(|(actual, guessed)| if actual != guessed { Some(actual) } else { None })
            .collect();

        guess.char_indices()
            .map(|(i, c)| if c == self.target[i] {
                RES_CORRECT
            } else {
                match unmatched.iter().position(|x| *x == c) {
                    Some(idx) => {
                        unmatched.swap_remove(idx);
                        RES_PRESENT
                    }
                    None => RES_WRONG,
                }
            })
            .collect::<Vec<u8>>()
            .try_into().unwrap()
    }
}

fn render_boxed_word<I>(stdout: &mut Stdout, word: &str, styles: I) -> io::Result<()>
    where I: Iterator<Item=ContentStyle> {
    let (cols, _) = terminal::size()?;
    let x = (cols / 2) - (4 * word.len() as u16 - 1) / 2;
    for ((ci, c), style) in word.char_indices().zip(styles) {
        let cx = x + (ci as u16 * 4);
        queue!(stdout, MoveToColumn(cx))?;
        draw_charbox(stdout, c, style)?;
    }
    queue!(stdout, MoveDown(3))
}

fn render_message_centered(stdout: &mut Stdout, message: StyledContent<&str>) -> io::Result<()> {
    queue!(stdout, MoveToColumn(terminal::size()?.0 / 2 - message.content().len() as u16 / 2), PrintStyledContent(message))
}

impl Renderable for RusdleState {
    fn render(&self, stdout: &mut Stdout) -> io::Result<()> {
        queue!(stdout, Clear(ClearType::All), ResetColor, MoveTo(0, 0))?;
        let (cols, _) = terminal::size()?;

        render_boxed_word(stdout, "RUSDLE", repeat(ContentStyle::new().blue().bold().italic()))?;
        render_message_centered(stdout, "Wordle in Rust".bold())?;
        queue!(stdout, MoveDown(1))?;

        let mut render_guess = |guess: &str, result: &[u8; 5]|
            render_boxed_word(stdout, &guess, result.iter().map(|r| result_colours(*r)));
        for (guess, result) in self.guesses.iter() { render_guess(guess, result)?; }

        if !self.is_over() {
            render_guess(&format!("{}_    ", self.entry)[..5], &[RES_DEFAULT; 5])?;
            for _ in self.guesses.len()..5 {
                render_guess("     ", &[RES_DEFAULT; 5])?;
            }
        } else {
            for _ in self.guesses.len()..6 {
                render_guess("     ", &[RES_DEFAULT; 5])?;
            }
        }

        let mut render_keyrow = |row: &str| -> io::Result<()> {
            queue!(stdout, MoveToColumn(cols / 2 - row.len() as u16), MoveDown(1))?;
            let mut prev_style = ContentStyle::new();
            for c in row.chars() {
                if c == ' ' { continue; }
                let style = match self.clues.get(&c).map(|r| *r) {
                    Some(RES_WRONG) => ContentStyle::new().dark_grey().on(Color::from((32, 32, 32))),
                    Some(r) => result_colours(r),
                    None => ContentStyle::new().black().on_dark_grey()
                };

                queue!(stdout,
                    PrintStyledContent(prev_style.clone().with(style.background_color.unwrap_or(Color::White)).apply('▐')),
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
        queue!(stdout, MoveDown(2))?;

        let message = if self.is_over() {
            if self.is_win() { "Winner!".green() } else { "Loser!".red() }
        } else {
            match &self.last_error {
                Some(msg) => msg.as_str().dark_yellow(),
                _ => "".stylize(),
            }
        };

        render_message_centered(stdout, message.slow_blink())?;
        execute!(stdout, MoveDown(1), MoveToColumn(0))
    }
}

trait Renderable {
    fn render(&self, stdout: &mut Stdout) -> io::Result<()>;
}

/**
 * Draws a box at the current location and returns to the original location
 */
#[allow(dead_code)]
fn draw_box(stdout: &mut Stdout, size: (u16, u16), style: ContentStyle) -> io::Result<()> {
    let (w, h) = size;
    let pad = (w - 2) as usize;
    for i in 1..=h {
        queue!(stdout,
            PrintStyledContent(style.dim().apply(
                match i {
                    1 => format!("{:\u{2584}<1$}", "", w as usize),
                    x if x == h => format!("{:\u{2580}<1$}", "", w as usize),
                    _ => format!("\u{258c}{: <1$}\u{2590}", "", pad)
                }
            )),
            MoveLeft(w),
            MoveDown(1),
        )?
    }
    queue!(stdout, MoveUp(h))
}

fn draw_charbox(stdout: &mut Stdout, c: char, style: ContentStyle) -> io::Result<()> {
    match style {
        ContentStyle { background_color: None, .. } => queue!(stdout,
            PrintStyledContent(style.apply("\u{250c}\u{2500}\u{2510}")), MoveDown(1), MoveLeft(3),
            PrintStyledContent(style.apply("\u{2502} \u{2502}")), MoveDown(1), MoveLeft(3),
            PrintStyledContent(style.apply("\u{2514}\u{2500}\u{2518}")), MoveUp(1), MoveLeft(2),
            PrintStyledContent(style.apply(c)), MoveUp(1), MoveLeft(1),
        ),
        _ => queue!(stdout,
            PrintStyledContent(style.black().negative().apply("\u{2584}\u{2584}\u{2584}")), MoveDown(1), MoveLeft(3),
            PrintStyledContent(style.apply(format!(" {} ", c))), MoveDown(1), MoveLeft(3),
            PrintStyledContent(style.black().negative().apply("\u{2580}\u{2580}\u{2580}")), MoveUp(2), MoveLeft(2),
        )
    }
}

const RES_DEFAULT: u8 = 0;
const RES_WRONG: u8 = 1;
const RES_PRESENT: u8 = 2;
const RES_CORRECT: u8 = 3;

fn result_colours(r: u8) -> ContentStyle {
    match r {
        RES_DEFAULT => ContentStyle::new().white().on(Color::from((32, 32, 32))).bold(),
        RES_WRONG => ContentStyle::new().black().on_dark_grey(),
        RES_PRESENT => ContentStyle::new().black().on_dark_yellow().bold(),
        RES_CORRECT => ContentStyle::new().black().on_dark_green().bold(),
        _ => panic!("unknown char result {}", r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_words() -> WordSet { WordSet::load(None::<&str>, None::<&str>).unwrap() }

    fn test_game(target: &str) -> RusdleState {
        RusdleState::new_with_target(default_words(), target)
    }

    fn result(code: &str) -> [u8; 5] {
        code.chars()
            .map(|c| {
                match c {
                    '!' => RES_CORRECT,
                    '?' => RES_PRESENT,
                    'x' => RES_WRONG,
                    _ => panic!("unknown code char {}", c)
                }
            })
            .collect::<Vec<u8>>().as_slice()
            .try_into().unwrap()
    }

    #[test]
    fn compare_guess_confirms_all_match() {
        assert_eq!(test_game("MATCH").compare_guess("MATCH"), result("!!!!!"))
    }

    #[test]
    fn compare_guess_allows_double_correct() {
        assert_eq!(test_game("NOOBS").compare_guess("ROOTY"), result("x!!xx"))
    }

    #[test]
    fn compare_guess_allows_double_present() {
        assert_eq!(test_game("NOOBS").compare_guess("IGLOO"), result("xxx??"))
    }

    #[test]
    fn compare_guess_isnt_greedy() {
        assert_eq!(test_game("FRAME").compare_guess("ELIDE"), result("xxxx!"))
    }
}