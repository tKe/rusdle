use std::io;
use std::time::SystemTime;
use crossterm::{cursor, style::Print, terminal::{Clear, ClearType}, event::{read, Event}, execute, queue};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Color, PrintStyledContent, ResetColor, SetForegroundColor, Stylize};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use std::{
    fs::File,
    io::{prelude::*, BufReader},
    path::Path,
};
use std::fmt::{Debug, Formatter};

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
        let now = SystemTime::now();
        let elapsed = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        let idx = (elapsed.as_secs() - 1624057200) / 86400;

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
}

impl RusdleState {
    pub fn new(words: WordSet) -> Self {
        let target = words.word_of_the_day().chars().collect();
        Self {
            words,
            target,
            last_error: None,
            entry: String::new(),
            guesses: Vec::new(),
        }
    }
}

impl RusdleState {
    fn render(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        queue!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

        for (guess_idx, (guess, result)) in self.guesses.iter().enumerate() {
            queue!(stdout, Print(format!("  Guess {}: ", guess_idx + 1)))?;
            for s in guess.chars().zip(result)
                .map(|(c, r)| PrintStyledContent(c.on(result_colour(*r)))) {
                queue!(stdout, s)?;
            };
            queue!(stdout, ResetColor, cursor::MoveToNextLine(1), Clear(ClearType::UntilNewLine))?
        }

        match &self.last_error {
            None => {}
            Some(msg) => queue!(stdout,
                SetForegroundColor(Color::DarkYellow),
                Print(msg),
                ResetColor,
                cursor::MoveToNextLine(1), Clear(ClearType::UntilNewLine))?
        }

        if self.is_over() {
            queue!(stdout, Print(if self.is_win() { "Winner!" } else { "Loser!" } ))?;
        } else {
            queue!(stdout,
                Print("New guess: "),
                Print(self.entry.as_str()),
            )?;
        }
        execute!(stdout, cursor::MoveToNextLine(1))
    }

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

fn result_colour(r: u8) -> Color {
    match r {
        1 => Color::DarkYellow,
        2 => Color::DarkGreen,
        _ => Color::DarkGrey,
    }
}