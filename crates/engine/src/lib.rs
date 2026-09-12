//! Frontend-agnostic input engine interface.
//!
//! The Wayland frontend translates raw key events into [`KeyInput`], hands them
//! to an [`InputEngine`], and renders whatever [`InputEngine::preedit`] and
//! [`InputEngine::candidates`] report afterwards.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

impl Modifiers {
    pub fn any(&self) -> bool {
        self.shift || self.ctrl || self.alt || self.logo
    }
}

/// Keys the engine cares about beyond plain characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Backspace,
    Escape,
    Space,
    Enter,
    Tab,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    /// Shift, Ctrl, Alt, Super and friends.
    Modifier,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInput {
    pub key: Key,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    /// Full input code for this candidate, shown as a hint (e.g. Cangjie radicals).
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Preedit {
    pub text: String,
    /// Byte offset of the cursor within `text`.
    pub cursor: usize,
}

/// What the engine did with a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// Engine did not handle it; frontend should forward it to the application.
    Ignored,
    /// Engine consumed it and its visible state may have changed.
    Consumed,
    /// Engine consumed it but the input was invalid; frontend may signal an error.
    Bell,
    /// Engine consumed it and wants `text` committed to the application.
    Commit(String),
    /// Commit `text`, then forward the key to the application as if ignored.
    CommitAndForward(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageInfo {
    pub page: usize,
    pub total_pages: usize,
    pub page_size: usize,
}

pub trait InputEngine {
    fn name(&self) -> &str;

    fn process_key(&mut self, input: KeyInput) -> Response;

    fn preedit(&self) -> Preedit;

    /// Candidates on the current page.
    fn candidates(&self) -> Vec<Candidate>;

    fn page_info(&self) -> PageInfo;

    /// Index within the current page that is highlighted.
    fn selected(&self) -> usize;

    /// Discard any in-progress composition.
    fn reset(&mut self);

    fn is_composing(&self) -> bool {
        !self.preedit().text.is_empty()
    }
}
