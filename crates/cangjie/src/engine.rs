//! Key handling mirrors ibus-cangjie so that Hong Kong users get the
//! behaviour they already know: Space is the "power key", typing a new key
//! auto-commits the first candidate, and full-width punctuation comes from
//! libcangjie's shortcode table.

use std::collections::{HashMap, HashSet};

use popeinput_engine::{Candidate, InputEngine, Key, KeyInput, PageInfo, Preedit, Response};

use crate::db::{CangjieDb, CangjieError, Char, Filter, Version};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Full Cangjie: up to five keys, `*` wildcard, lookup on Space.
    Cangjie,
    /// Quick (速成): first and last key, lookup as soon as two keys are typed.
    Quick,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub version: Version,
    pub filter: Filter,
    pub mode: Mode,
    pub page_size: usize,
    /// Commit full-width variants of numbers, punctuation and space.
    pub fullwidth_chars: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: Version::V5,
            filter: Filter::HONG_KONG,
            mode: Mode::Cangjie,
            page_size: 9,
            fullwidth_chars: true,
        }
    }
}

pub struct CangjieEngine {
    db: CangjieDb,
    cfg: Config,
    radicals: HashMap<char, String>,
    input: String,
    candidates: Vec<Candidate>,
    page: usize,
    /// Set after a failed lookup: the bad input stays visible until the next key.
    clear_on_next_input: bool,
}

impl CangjieEngine {
    pub fn new(cfg: Config) -> Result<Self, CangjieError> {
        let mut db = CangjieDb::open(cfg.version, cfg.filter)?;
        let radicals = ('a'..='z')
            .filter_map(|k| db.radical(k).map(|r| (k, r)))
            .collect();
        Ok(CangjieEngine {
            db,
            cfg,
            radicals,
            input: String::new(),
            candidates: Vec::new(),
            page: 0,
            clear_on_next_input: false,
        })
    }

    pub fn mode(&self) -> Mode {
        self.cfg.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.cfg.mode = mode;
        self.reset();
    }

    pub fn set_fullwidth_chars(&mut self, on: bool) {
        self.cfg.fullwidth_chars = on;
    }

    fn max_input_len(&self) -> usize {
        match self.cfg.mode {
            Mode::Cangjie => 5,
            Mode::Quick => 2,
        }
    }

    fn to_radicals(&self, code: &str) -> String {
        code.chars()
            .map(|c| match c {
                '*' => "＊".to_string(),
                c => self.radicals.get(&c).cloned().unwrap_or_else(|| c.to_string()),
            })
            .collect()
    }

    fn is_input_key(&mut self, c: char) -> bool {
        c.is_ascii_lowercase() && self.db.is_input_key(c)
    }

    fn set_candidates(&mut self, chars: Vec<Char>) {
        let mut chars = chars;
        chars.sort_by_key(|c| std::cmp::Reverse(c.frequency));
        let mut seen = HashSet::new();
        self.candidates = chars
            .into_iter()
            .filter(|c| seen.insert(c.chchar.clone()))
            .map(|c| Candidate {
                hint: Some(self.to_radicals(&c.code)),
                text: c.chchar,
            })
            .collect();
        self.page = 0;
    }

    /// Look up `code`, auto-committing if there is exactly one match.
    fn lookup(&mut self, code: &str, by_shortcode: bool) -> Response {
        let result = if by_shortcode {
            self.db.characters_by_shortcode(code)
        } else {
            self.db.characters(code)
        };
        match result {
            Ok(chars) if !chars.is_empty() => {
                self.set_candidates(chars);
                if self.candidates.len() == 1 {
                    self.select(0)
                } else {
                    Response::Consumed
                }
            }
            Ok(_) => {
                self.candidates.clear();
                self.clear_on_next_input = true;
                Response::Bell
            }
            Err(CangjieError::Invalid) => {
                self.candidates.clear();
                self.clear_on_next_input = true;
                Response::Bell
            }
            Err(e) => {
                log::error!("lookup for {code:?} failed: {e}");
                self.candidates.clear();
                Response::Bell
            }
        }
    }

    fn lookup_current(&mut self) -> Response {
        let code = match self.cfg.mode {
            Mode::Quick => {
                let keys: Vec<String> = self.input.chars().map(String::from).collect();
                keys.join("*")
            }
            Mode::Cangjie => self.input.clone(),
        };
        // A lone non-Cangjie key means the user typed punctuation whose
        // full-width lookup returned several choices.
        let by_shortcode = self.input.chars().count() == 1
            && !self.is_input_key(self.input.chars().next().unwrap());
        self.lookup(&code, by_shortcode)
    }

    fn total_pages(&self) -> usize {
        self.candidates.len().div_ceil(self.cfg.page_size).max(1)
    }

    fn page_slice(&self) -> &[Candidate] {
        let start = (self.page * self.cfg.page_size).min(self.candidates.len());
        let end = (start + self.cfg.page_size).min(self.candidates.len());
        &self.candidates[start..end]
    }

    /// Commit the candidate at `index` on the current page.
    fn select(&mut self, index: usize) -> Response {
        match self.page_slice().get(index) {
            Some(c) => {
                let text = c.text.clone();
                self.reset();
                Response::Commit(text)
            }
            None => Response::Consumed,
        }
    }

    fn push_input(&mut self, c: char) -> Result<(), ()> {
        if self.clear_on_next_input {
            self.reset();
        }
        if self.input.chars().count() >= self.max_input_len() {
            return Err(());
        }
        self.input.push(c);
        Ok(())
    }

    fn input_char(&mut self, c: char) -> Response {
        let committed = if !self.candidates.is_empty() {
            match self.select(0) {
                Response::Commit(t) => Some(t),
                _ => None,
            }
        } else {
            None
        };

        let mut response = match self.push_input(c) {
            Ok(()) => Response::Consumed,
            Err(()) => Response::Bell,
        };

        if self.cfg.mode == Mode::Quick && self.input.chars().count() == self.max_input_len() {
            response = self.lookup_current();
        }

        merge_commit(committed, response)
    }

    fn space(&mut self) -> Response {
        if self.input.is_empty() {
            return self.fullwidth_char(' ');
        }
        if self.candidates.is_empty() {
            return self.lookup_current();
        }
        if self.candidates.len() <= self.cfg.page_size {
            return self.select(0);
        }
        self.page_down()
    }

    fn page_down(&mut self) -> Response {
        if self.candidates.is_empty() {
            return Response::Ignored;
        }
        self.page = (self.page + 1) % self.total_pages();
        Response::Consumed
    }

    fn page_up(&mut self) -> Response {
        if self.candidates.is_empty() {
            return Response::Ignored;
        }
        self.page = (self.page + self.total_pages() - 1) % self.total_pages();
        Response::Consumed
    }

    fn number(&mut self, d: char) -> Response {
        if !self.candidates.is_empty() {
            return match d.to_digit(10) {
                Some(n) if n >= 1 => self.select(n as usize - 1),
                _ => Response::Consumed,
            };
        }
        self.fullwidth_char(d)
    }

    fn other_key(&mut self, c: char) -> Response {
        let mut committed = None;
        if self.candidates.is_empty() && !self.input.is_empty() {
            match self.lookup_current() {
                Response::Commit(t) => committed = Some(t),
                Response::Bell => return Response::Bell,
                _ => {}
            }
        }
        if !self.candidates.is_empty() {
            if let Response::Commit(t) = self.select(0) {
                committed = Some(t);
            }
        }
        merge_commit(committed, self.fullwidth_char(c))
    }

    fn fullwidth_char(&mut self, c: char) -> Response {
        if !self.cfg.fullwidth_chars {
            return Response::Ignored;
        }
        if self.push_input(c).is_err() {
            return Response::Bell;
        }
        let code = c.to_string();
        match self.lookup(&code, true) {
            Response::Bell => {
                self.reset();
                Response::Ignored
            }
            r => r,
        }
    }
}

/// Fold an auto-commit that happened earlier in the key's handling into the
/// final response for that key.
fn merge_commit(committed: Option<String>, response: Response) -> Response {
    let Some(text) = committed else { return response };
    match response {
        Response::Ignored => Response::CommitAndForward(text),
        Response::Consumed | Response::Bell => Response::Commit(text),
        Response::Commit(t) => Response::Commit(text + &t),
        Response::CommitAndForward(t) => Response::CommitAndForward(text + &t),
    }
}

impl InputEngine for CangjieEngine {
    fn name(&self) -> &str {
        match self.cfg.mode {
            Mode::Cangjie => "倉頡",
            Mode::Quick => "速成",
        }
    }

    fn process_key(&mut self, input: KeyInput) -> Response {
        let m = input.modifiers;
        if m.ctrl || m.alt || m.logo {
            return Response::Ignored;
        }

        match input.key {
            Key::Escape => {
                if self.input.is_empty() {
                    Response::Ignored
                } else {
                    self.reset();
                    Response::Consumed
                }
            }
            Key::Space => self.space(),
            Key::PageDown | Key::Down => self.page_down(),
            Key::PageUp | Key::Up => self.page_up(),
            Key::Backspace => {
                if self.input.is_empty() {
                    return Response::Ignored;
                }
                self.clear_on_next_input = false;
                self.input.pop();
                self.candidates.clear();
                self.page = 0;
                Response::Consumed
            }
            Key::Char(d) if d.is_ascii_digit() => self.number(d),
            Key::Char('*') => {
                if self.cfg.mode == Mode::Cangjie && !self.input.is_empty() {
                    self.input_char('*')
                } else {
                    self.other_key('*')
                }
            }
            Key::Char(c) if self.is_input_key(c) => self.input_char(c),
            Key::Char(c) => self.other_key(c),
            Key::Enter | Key::Tab | Key::Left | Key::Right | Key::Modifier(_) | Key::Other => {
                Response::Ignored
            }
        }
    }

    fn preedit(&self) -> Preedit {
        let text = self.to_radicals(&self.input);
        let cursor = text.len();
        Preedit { text, cursor }
    }

    fn candidates(&self) -> Vec<Candidate> {
        self.page_slice().to_vec()
    }

    fn page_info(&self) -> PageInfo {
        PageInfo {
            page: self.page,
            total_pages: Some(self.total_pages()),
            has_next: self.page + 1 < self.total_pages(),
            page_size: self.cfg.page_size,
        }
    }

    fn selected(&self) -> usize {
        0
    }

    fn reset(&mut self) {
        self.input.clear();
        self.candidates.clear();
        self.page = 0;
        self.clear_on_next_input = false;
    }

    fn is_composing(&self) -> bool {
        !self.input.is_empty()
    }
}
