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
use std::collections::HashMap;
use chrono::{DateTime, Local, TimeZone};
use crossterm::{
    cursor::{
        self,
        MoveTo,
        MoveToRow,
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
        Print,
        PrintStyledContent,
        ResetColor,
        Stylize,
    },
    terminal::{
        self,
        Clear,
        ClearType,
        disable_raw_mode,
        enable_raw_mode,
    },
    style::ContentStyle,
};
use crossterm::cursor::{MoveDown, MoveToColumn};
use crossterm::style::StyledContent;

fn lines_from_file(filename: impl AsRef<Path>) -> io::Result<Vec<String>> {
    let file = File::open(filename)?;
    BufReader::new(file).lines().collect()
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, cursor::Hide)?;

    let mut game = RusdleState::new(
        WordSet::load("./data/wordlist.txt", "./data/guesses.txt")?
    );

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
                    modifiers: KeyModifiers::NONE,
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
        cursor::Show)?;
    Ok(())
}

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
    fn load(wordlist_path: &str, guesses_path: &str) -> io::Result<Self> {
        Ok(WordSet {
            wordlist: lines_from_file(wordlist_path)?,
            valid_guesses: lines_from_file(guesses_path)?,
        })
    }

    fn is_valid(&self, word: &String) -> bool {
        self.wordlist.contains(word) || self.valid_guesses.contains(word)
    }

    fn word_of_the_day(&self) -> String {
        let epoch: DateTime<Local> = Local.ymd(2021, 6, 19).and_hms(0, 0, 0);
        let idx = (Local::now().date().and_hms(0, 0, 0).timestamp() - epoch.timestamp()) / 86400;
        self.wordlist.iter().nth(idx as usize % self.wordlist.len()).unwrap().clone()
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

impl RusdleState {
    pub fn new(words: WordSet) -> Self {
        let target = words.word_of_the_day().chars().collect();
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
            KeyCode::Char(char) => if self.entry.len() < 5 {
                self.entry.push(char)
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
            Some((_, [2, 2, 2, 2, 2])) => true,
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
                .for_each(|(c, r)| { self.clues.insert(c, r); });

            self.guesses.push((guess, result));
            self.entry.clear()
        }
    }

    fn compare_guess(&mut self, guess: &String) -> [u8; 5] {
        let mut avail: Vec<char> = self.target.clone();
        guess.char_indices()
            .map(|(i, c)| {
                let loc = avail.iter().position(|x| *x == c);
                match loc {
                    None => 0,
                    Some(idx) => {
                        avail.swap_remove(idx);
                        if c == self.target[i] { 2 } else { 1 }
                    }
                }
            })
            .collect::<Vec<u8>>()
            .try_into().unwrap()
    }
}

const EMPTY_RESULT: [u8; 5] = [0; 5];

fn render_boxed_word<I>(stdout: &mut Stdout, word: &str, styles: I) -> io::Result<()>
    where I: Iterator<Item=(ContentStyle, ContentStyle)> {
    let (_, y) = cursor::position()?;
    let (cols, _) = terminal::size()?;
    let x = (cols / 2) - (4 * word.len() as u16) / 2;
    for ((ci, c), (box_style, text_style)) in word.char_indices().zip(styles) {
        let cx = x + (ci as u16 * 4);
        draw_box(stdout, (cx, y), (3, 3), box_style)?;
        queue!(stdout, MoveTo(cx + 1, y + 1),
            PrintStyledContent(text_style.apply(c)),
            MoveDown(2), MoveToColumn(x))?;
    }
    Ok(())
}

fn render_message_centered(stdout: &mut Stdout, message: StyledContent<&str>) -> io::Result<()> {
    queue!(stdout, MoveToColumn(terminal::size()?.0 / 2 - message.content().len() as u16 / 2), PrintStyledContent(message))
}

impl Renderable for RusdleState {
    fn render(&self, stdout: &mut Stdout) -> io::Result<()> {
        queue!(stdout, Clear(ClearType::All), ResetColor, MoveToRow(1))?;
        let (cols, rows) = terminal::size()?;

        render_boxed_word(stdout, "RUSDLE", repeat((ContentStyle::new().blue().bold(), ContentStyle::new().white().bold().italic())))?;
        render_message_centered(stdout, "Wordle in Rust".bold())?;
        queue!(stdout, MoveDown(2))?;

        let mut render_guess = |guess: &str, result: &[u8; 5]|
            render_boxed_word(stdout, &guess, result.iter().map(|r| result_colours(*r)));
        for (guess, result) in self.guesses.iter() { render_guess(guess, result)?; }
        if !self.is_over() {
            render_guess(&format!("{}_    ", self.entry)[..5], &[3u8; 5])?;
            for _ in self.guesses.len()..5 {
                render_guess("     ", &EMPTY_RESULT)?;
            }

            let mut render_keyrow = |row: &str| {
                queue!(stdout, MoveToColumn(cols / 2 - row.len() as u16))?;
                for c in row.chars() {
                    queue!(stdout, Print(" "), PrintStyledContent(result_colours(*self.clues.get(&c).unwrap_or(&3)).0.apply(c)))?;
                }
                queue!(stdout, MoveDown(1))
            };

            render_keyrow("qwertyuiop")?;
            render_keyrow("asdfghjkl")?;
            render_keyrow("zxcvbnm")?;
        }

        let message = if self.is_over() {
            if self.is_win() { "Winner!".green() } else { "Loser!".red() }
        } else {
            match &self.last_error {
                Some(msg) => msg.as_str().dark_yellow(),
                _ => "".stylize(),
            }
        };

        render_message_centered(stdout, message.slow_blink())?;
        execute!(stdout, MoveToRow(rows))
    }
}

trait Renderable {
    fn render(&self, stdout: &mut Stdout) -> io::Result<()>;
}

fn draw_box(stdout: &mut Stdout, at: (u16, u16), size: (u16, u16), style: ContentStyle) -> io::Result<()> {
    let (x, y) = at;
    let (w, h) = size;
    let pad = (w - 2) as usize;
    queue!(stdout,
        MoveTo(x, y),
        PrintStyledContent(style.dim().apply(format!("\u{256d}{:\u{2500}<1$}\u{256e}", "", pad))),
    )?;
    for i in 1..h {
        queue!(stdout,
            MoveTo(x, y + i),
            PrintStyledContent(style.dim().apply(format!("\u{2502}{: <1$}\u{2502}", "", pad))),
        )?;
    }
    queue!(stdout,
        MoveTo(x, y + h - 1),
        PrintStyledContent(style.dim().apply(format!("\u{2570}{:\u{2500}<1$}\u{256f}", "", pad))),
        ResetColor
    )
}

fn result_colours(r: u8) -> (ContentStyle, ContentStyle) {
    match r {
        1 => {
            let s = ContentStyle::new().yellow().bold();
            (s, s)
        }
        2 => {
            let s = ContentStyle::new().green().bold();
            (s, s)
        }
        3 => {
            let s = ContentStyle::new().grey().bold();
            (s, s)
        }
        _ => {
            let s = ContentStyle::new().dark_grey();
            (s, s)
        }
    }
}