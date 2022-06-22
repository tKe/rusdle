use crate::MutVecExt;
use chrono::{DateTime, Local, TimeZone};
use clap::ArgEnum;
use rand::seq::SliceRandom;
use std::collections::HashMap;

use std::{
    fmt::{Debug, Formatter},
    fs::File,
    io::{self, prelude::*, BufReader},
    path::Path,
};

#[derive(Clone)]
pub struct WordSet {
    wordlist: Vec<String>,
    valid_guesses: Vec<String>,
}

impl Debug for WordSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("WordSet")
    }
}

impl WordSet {
    pub fn load(
        wordlist_path: Option<impl AsRef<Path>>,
        guesses_path: Option<impl AsRef<Path>>,
    ) -> io::Result<Self> {
        let wordlist = wordlist_path
            .map(|p| lines_from_file(p))
            .unwrap_or(Ok(include_str!("../data/wordlist.txt")
                .lines()
                .map(|s| String::from(s))
                .collect()))?;
        let valid_guesses =
            guesses_path
                .map(|p| lines_from_file(p))
                .unwrap_or(Ok(include_str!("../data/guesses.txt")
                    .lines()
                    .map(|s| String::from(s))
                    .collect()))?;

        Ok(WordSet {
            wordlist,
            valid_guesses,
        })
    }

    fn is_valid(&self, word: &str) -> bool {
        let needle = &word.to_lowercase();
        self.wordlist.contains(needle) || self.valid_guesses.contains(needle)
    }

    fn word_of_the_day(&self) -> String {
        let epoch: DateTime<Local> = Local.ymd(2021, 6, 19).and_hms(0, 0, 0);
        let idx = (Local::now().date().and_hms(0, 0, 0).timestamp() - epoch.timestamp()) / 86400;
        self.wordlist
            .get(idx as usize % self.wordlist.len())
            .unwrap()
            .to_ascii_uppercase()
    }

    fn random_word(&self) -> String {
        self.wordlist
            .choose(&mut rand::thread_rng())
            .unwrap()
            .to_ascii_uppercase()
    }
}

#[derive(Debug)]
pub(crate) struct RusdleState {
    words: WordSet,
    target: Vec<char>,
    pub(crate) entry: String,
    pub(crate) last_error: Option<String>,
    pub(crate) guesses: Vec<(String, [u8; 5])>,
    pub(crate) clues: HashMap<char, u8>,
}

#[derive(ArgEnum, Clone)]
pub enum GameMode {
    Wordle,
    RandomWord,
}

pub enum GameInput {
    Delete,
    Submit,
    Input(char),
}

impl RusdleState {
    pub fn new(words: WordSet, mode: GameMode) -> Self {
        let word = match mode {
            GameMode::RandomWord => words.random_word(),
            GameMode::Wordle => words.word_of_the_day(),
        };
        Self::new_with_target(words, &word)
    }

    pub fn new_with_target(words: WordSet, target: &str) -> Self {
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

    pub fn handle_input(&mut self, input: GameInput) {
        match input {
            GameInput::Input(c) => {
                if self.entry.len() < 5 {
                    self.entry.push(c.to_ascii_uppercase())
                }
            }
            GameInput::Delete => {
                if self.entry.len() > 0 {
                    self.entry.pop();
                }
            }
            GameInput::Submit => {
                if self.entry.len() == 5 {
                    self.process_guess();
                }
            }
        }
    }

    pub fn is_over(&self) -> bool {
        self.guesses.len() == 6 || self.is_win()
    }

    pub fn is_win(&self) -> bool {
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

            guess.chars().zip(result).for_each(|(c, r)| {
                let clue = self.clues.entry(c).or_insert(r);
                if r > *clue {
                    *clue = r
                }
            });

            self.guesses.push((guess, result));
            self.entry.clear()
        }
    }

    fn compare_guess(&mut self, guess: &str) -> [u8; 5] {
        let mut unmatched: Vec<char> = self
            .target
            .iter()
            .cloned()
            .zip(guess.chars())
            .filter_map(|(actual, guessed)| {
                if actual != guessed {
                    Some(actual)
                } else {
                    None
                }
            })
            .collect();

        guess
            .char_indices()
            .map(|(i, c)| {
                if c == self.target[i] {
                    RES_CORRECT
                } else if unmatched.remove_item(c) {
                    RES_PRESENT
                } else {
                    RES_WRONG
                }
            })
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap()
    }
}

pub const RES_DEFAULT: u8 = 0;
pub const RES_WRONG: u8 = 1;
pub const RES_PRESENT: u8 = 2;
pub const RES_CORRECT: u8 = 3;

fn lines_from_file(filename: impl AsRef<Path>) -> io::Result<Vec<String>> {
    let file = File::open(filename)?;
    BufReader::new(file).lines().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_words() -> WordSet {
        WordSet::load(None::<&str>, None::<&str>).unwrap()
    }

    fn test_game(target: &str) -> RusdleState {
        RusdleState::new_with_target(default_words(), target)
    }

    fn result(code: &str) -> [u8; 5] {
        code.chars()
            .map(|c| match c {
                '!' => RES_CORRECT,
                '?' => RES_PRESENT,
                'x' => RES_WRONG,
                _ => panic!("unknown code char {}", c),
            })
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap()
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
