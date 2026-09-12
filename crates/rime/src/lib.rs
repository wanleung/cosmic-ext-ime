//! RIME engine: drives a librime session and exposes it as an [`InputEngine`].
//!
//! librime is process-global (one `initialize`, many sessions), so the
//! library is set up once and every `RimeEngine` owns just a session.

use std::ffi::{CStr, CString};
use std::os::raw::c_int;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use cosmic_ext_ime_engine::{
    Candidate, InputEngine, Key, KeyInput, ModifierKey, PageInfo, Preedit, Response,
};
use rime_sys as sys;

macro_rules! api {
    ($api:expr, $f:ident) => {
        $api.$f
            .expect(concat!("librime API missing ", stringify!($f)))
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub shared_data_dir: PathBuf,
    pub user_data_dir: PathBuf,
    /// Schema id to select; `None` keeps RIME's default (the first listed).
    pub schema: Option<String>,
    /// Used when generating the initial `default.custom.yaml`.
    pub page_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        Config {
            shared_data_dir: ["/usr/share/rime-data", "/usr/local/share/rime-data"]
                .iter()
                .map(PathBuf::from)
                .find(|p| p.is_dir())
                .unwrap_or_else(|| PathBuf::from("/usr/share/rime-data")),
            user_data_dir: data_home.join("cosmic-ext-ime").join("rime"),
            schema: None,
            page_size: 9,
        }
    }
}

struct Library {
    api: &'static sys::RimeApi,
    /// Keeps the strings handed to librime's traits alive for the process.
    _strings: Vec<CString>,
}

// librime's API table is immutable and its sessions are internally locked.
unsafe impl Send for Library {}
unsafe impl Sync for Library {}

static LIBRARY: OnceLock<Result<Library, String>> = OnceLock::new();
static SCHEMAS: Mutex<Vec<Schema>> = Mutex::new(Vec::new());

fn library(cfg: &Config) -> Result<&'static Library, String> {
    LIBRARY
        .get_or_init(|| init_library(cfg))
        .as_ref()
        .map_err(|e| e.clone())
}

fn init_library(cfg: &Config) -> Result<Library, String> {
    std::fs::create_dir_all(&cfg.user_data_dir)
        .map_err(|e| format!("creating {}: {e}", cfg.user_data_dir.display()))?;
    ensure_default_custom(cfg);

    // SAFETY: rime_get_api returns a pointer to a static table.
    let api: &'static sys::RimeApi = unsafe { sys::rime_get_api().as_ref() }
        .ok_or_else(|| "rime_get_api returned NULL".to_string())?;

    let strings: Vec<CString> = [
        cfg.shared_data_dir.to_string_lossy().into_owned(),
        cfg.user_data_dir.to_string_lossy().into_owned(),
        "cosmic-ext-ime".to_string(),
        "cosmic-ext-ime".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        "rime.cosmic-ext-ime".to_string(),
    ]
    .into_iter()
    .map(|s| CString::new(s).unwrap())
    .collect();

    // SAFETY: traits is zeroed then sized per RIME_STRUCT_INIT; librime copies the strings.
    let mut traits: sys::RimeTraits = unsafe { std::mem::zeroed() };
    traits.data_size =
        (std::mem::size_of::<sys::RimeTraits>() - std::mem::size_of::<c_int>()) as c_int;
    traits.shared_data_dir = strings[0].as_ptr();
    traits.user_data_dir = strings[1].as_ptr();
    traits.distribution_name = strings[2].as_ptr();
    traits.distribution_code_name = strings[3].as_ptr();
    traits.distribution_version = strings[4].as_ptr();
    traits.app_name = strings[5].as_ptr();
    traits.min_log_level = 2;

    log::info!(
        "initialising librime (shared {}, user {})",
        cfg.shared_data_dir.display(),
        cfg.user_data_dir.display()
    );
    // SAFETY: standard librime start-up sequence on a valid traits struct.
    unsafe {
        api!(api, setup)(&mut traits);
        api!(api, initialize)(&mut traits);
        if api!(api, start_maintenance)(0) != 0 {
            log::info!("librime is deploying schemas; first run can take a while");
            api!(api, join_maintenance_thread)();
        }
    }

    let lib = Library {
        api,
        _strings: strings,
    };
    *SCHEMAS.lock().unwrap() = list_schemas(&lib);
    Ok(lib)
}

/// RIME only builds schemas listed in `default.yaml`'s `schema_list`. Give
/// new users every installed schema instead of rime-prelude's Pinyin-centric
/// default; the file is theirs to edit afterwards.
fn ensure_default_custom(cfg: &Config) {
    let path = cfg.user_data_dir.join("default.custom.yaml");
    if path.exists() {
        return;
    }
    let mut ids: Vec<String> = Vec::new();
    for dir in [&cfg.shared_data_dir, &cfg.user_data_dir] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".schema.yaml") {
                if !ids.iter().any(|i| i == id) {
                    ids.push(id.to_string());
                }
            }
        }
    }
    ids.sort();
    // Cangjie/Quick first for Hong Kong users, then everything else.
    ids.sort_by_key(|id| {
        !(id.starts_with("cangjie") || id.starts_with("quick") || id.contains("jyut"))
    });
    let mut yaml = String::from(
        "# Generated by cosmic-ext-ime on first run. Edit freely; it is never overwritten.\n\
         # Remove schemas you do not want from schema_list to speed up deployment.\n\
         patch:\n  schema_list:\n",
    );
    for id in &ids {
        yaml.push_str(&format!("    - schema: {id}\n"));
    }
    yaml.push_str(&format!(
        "  menu/page_size: {}\n",
        cfg.page_size.clamp(1, 9)
    ));
    if let Err(e) = std::fs::write(&path, yaml) {
        log::warn!("could not write {}: {e}", path.display());
    } else {
        log::info!("wrote {} with {} schemas", path.display(), ids.len());
    }
}

fn list_schemas(lib: &Library) -> Vec<Schema> {
    let mut out = Vec::new();
    let mut list = sys::RimeSchemaList {
        size: 0,
        list: std::ptr::null_mut(),
    };
    // SAFETY: list is filled by librime and freed with free_schema_list.
    unsafe {
        if api!(lib.api, get_schema_list)(&mut list) != 0 {
            for i in 0..list.size {
                let item = &*list.list.add(i);
                out.push(Schema {
                    id: cstr(item.schema_id),
                    name: cstr(item.name),
                });
            }
            api!(lib.api, free_schema_list)(&mut list);
        }
    }
    out
}

/// Schemas RIME has deployed, available after the first engine is created.
pub fn schemas() -> Vec<Schema> {
    SCHEMAS.lock().unwrap().clone()
}

/// Rebuild all schemas from the yaml sources (after the user edited
/// `*.custom.yaml`). Existing sessions keep old data; recreate engines after.
pub fn redeploy() -> Result<(), String> {
    let Some(lib) = LIBRARY.get().and_then(|r| r.as_ref().ok()) else {
        // Not initialised yet: the first engine created will deploy anyway.
        return Ok(());
    };
    log::info!("redeploying RIME schemas");
    // SAFETY: library is initialised; full maintenance is a supported call.
    unsafe {
        if api!(lib.api, start_maintenance)(1) != 0 {
            api!(lib.api, join_maintenance_thread)();
        }
    }
    *SCHEMAS.lock().unwrap() = list_schemas(lib);
    Ok(())
}

fn cstr(p: *const std::os::raw::c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        // SAFETY: librime hands out NUL-terminated strings.
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

#[derive(Default)]
struct Snapshot {
    preedit: Preedit,
    candidates: Vec<Candidate>,
    page: usize,
    has_next: bool,
    page_size: usize,
    selected: usize,
    composing: bool,
    schema_name: String,
}

pub struct RimeEngine {
    lib: &'static Library,
    session: sys::RimeSessionId,
    snapshot: Snapshot,
}

impl RimeEngine {
    pub fn new(cfg: Config) -> Result<Self, String> {
        let lib = library(&cfg)?;
        // SAFETY: library is initialised.
        let session = unsafe { api!(lib.api, create_session)() };
        if session == 0 {
            return Err("librime could not create a session (no schemas deployed?)".into());
        }
        let mut engine = RimeEngine {
            lib,
            session,
            snapshot: Snapshot::default(),
        };
        if let Some(id) = cfg.schema.as_deref().filter(|s| !s.is_empty()) {
            let cid = CString::new(id).map_err(|_| "invalid schema id")?;
            // SAFETY: valid session and C string.
            if unsafe { api!(lib.api, select_schema)(session, cid.as_ptr()) } == 0 {
                log::warn!("RIME schema {id:?} not available; using default");
            }
        }
        engine.refresh();
        log::info!(
            "RIME session ready, schema {:?}",
            engine.snapshot.schema_name
        );
        Ok(engine)
    }

    pub fn user_data_dir(cfg: &Config) -> &Path {
        &cfg.user_data_dir
    }

    fn refresh(&mut self) {
        let api = self.lib.api;
        let mut snap = Snapshot::default();

        // SAFETY: structs are zeroed and sized per RIME_STRUCT_INIT before use,
        // and freed with the matching free_* call.
        unsafe {
            let mut status: sys::RimeStatus = std::mem::zeroed();
            status.data_size =
                (std::mem::size_of::<sys::RimeStatus>() - std::mem::size_of::<c_int>()) as c_int;
            if api!(api, get_status)(self.session, &mut status) != 0 {
                snap.schema_name = cstr(status.schema_name);
                snap.composing = status.is_composing != 0;
                api!(api, free_status)(&mut status);
            }

            let mut ctx: sys::RimeContext = std::mem::zeroed();
            ctx.data_size =
                (std::mem::size_of::<sys::RimeContext>() - std::mem::size_of::<c_int>()) as c_int;
            if api!(api, get_context)(self.session, &mut ctx) != 0 {
                let text = cstr(ctx.composition.preedit);
                let cursor = (ctx.composition.cursor_pos.max(0) as usize).min(text.len());
                snap.preedit = Preedit { text, cursor };
                let menu = &ctx.menu;
                snap.page = menu.page_no.max(0) as usize;
                snap.has_next = menu.is_last_page == 0 && menu.num_candidates > 0;
                snap.page_size = menu.page_size.max(1) as usize;
                snap.selected = menu.highlighted_candidate_index.max(0) as usize;
                for i in 0..menu.num_candidates.max(0) as usize {
                    let c = &*menu.candidates.add(i);
                    let comment = cstr(c.comment);
                    snap.candidates.push(Candidate {
                        text: cstr(c.text),
                        hint: (!comment.is_empty()).then_some(comment),
                    });
                }
                api!(api, free_context)(&mut ctx);
            }
        }
        self.snapshot = snap;
    }

    fn send_key(&mut self, input: KeyInput, extra_mask: c_int) -> Response {
        let (sym, mask) = to_rime_key(&input);
        if sym == 0 {
            return Response::Ignored;
        }
        // SAFETY: valid session.
        let handled =
            unsafe { api!(self.lib.api, process_key)(self.session, sym, mask | extra_mask) } != 0;
        let commit = self.take_commit();
        self.refresh();
        match (handled, commit) {
            (true, Some(text)) => Response::Commit(text),
            (true, None) => Response::Consumed,
            (false, Some(text)) => Response::CommitAndForward(text),
            (false, None) => Response::Ignored,
        }
    }

    fn take_commit(&mut self) -> Option<String> {
        // SAFETY: zeroed + sized struct, freed after reading.
        unsafe {
            let mut commit: sys::RimeCommit = std::mem::zeroed();
            commit.data_size =
                (std::mem::size_of::<sys::RimeCommit>() - std::mem::size_of::<c_int>()) as c_int;
            if api!(self.lib.api, get_commit)(self.session, &mut commit) != 0 {
                let text = cstr(commit.text);
                api!(self.lib.api, free_commit)(&mut commit);
                (!text.is_empty()).then_some(text)
            } else {
                None
            }
        }
    }
}

impl Drop for RimeEngine {
    fn drop(&mut self) {
        // SAFETY: session was created by us and is destroyed once.
        unsafe { api!(self.lib.api, destroy_session)(self.session) };
    }
}

// X11 modifier bits, as in librime's key_table.h (C++ only, so not in the C API).
const SHIFT_MASK: c_int = 1 << 0;
const CONTROL_MASK: c_int = 1 << 2;
const ALT_MASK: c_int = 1 << 3;
const SUPER_MASK: c_int = 1 << 6;
const RELEASE_MASK: c_int = 1 << 30;

/// X11 keysym and modifier mask as librime expects them.
fn to_rime_key(input: &KeyInput) -> (c_int, c_int) {
    let sym: u32 = match input.key {
        Key::Char(c) if (c as u32) < 0x80 => c as u32,
        Key::Char(c) => 0x0100_0000 + c as u32,
        Key::Backspace => 0xff08,
        Key::Tab => 0xff09,
        Key::Enter => 0xff0d,
        Key::Escape => 0xff1b,
        Key::Space => 0x0020,
        Key::Left => 0xff51,
        Key::Up => 0xff52,
        Key::Right => 0xff53,
        Key::Down => 0xff54,
        Key::PageUp => 0xff55,
        Key::PageDown => 0xff56,
        Key::Modifier(m) => match m {
            ModifierKey::ShiftL => 0xffe1,
            ModifierKey::ShiftR => 0xffe2,
            ModifierKey::ControlL => 0xffe3,
            ModifierKey::ControlR => 0xffe4,
            ModifierKey::AltL => 0xffe9,
            ModifierKey::AltR => 0xffea,
            ModifierKey::SuperL => 0xffeb,
            ModifierKey::SuperR => 0xffec,
            ModifierKey::Other => 0,
        },
        Key::Other => 0,
    };
    let m = input.modifiers;
    let mut mask = 0;
    if m.shift {
        mask |= SHIFT_MASK;
    }
    if m.ctrl {
        mask |= CONTROL_MASK;
    }
    if m.alt {
        mask |= ALT_MASK;
    }
    if m.logo {
        mask |= SUPER_MASK;
    }
    (sym as c_int, mask)
}

impl InputEngine for RimeEngine {
    fn name(&self) -> &str {
        if self.snapshot.schema_name.is_empty() {
            "RIME"
        } else {
            &self.snapshot.schema_name
        }
    }

    fn process_key(&mut self, input: KeyInput) -> Response {
        self.send_key(input, 0)
    }

    fn release_key(&mut self, input: KeyInput) -> Response {
        self.send_key(input, RELEASE_MASK)
    }

    fn preedit(&self) -> Preedit {
        self.snapshot.preedit.clone()
    }

    fn candidates(&self) -> Vec<Candidate> {
        self.snapshot.candidates.clone()
    }

    fn page_info(&self) -> PageInfo {
        PageInfo {
            page: self.snapshot.page,
            total_pages: None,
            has_next: self.snapshot.has_next,
            page_size: self.snapshot.page_size,
        }
    }

    fn selected(&self) -> usize {
        self.snapshot.selected
    }

    fn reset(&mut self) {
        // SAFETY: valid session.
        unsafe { api!(self.lib.api, clear_composition)(self.session) };
        self.refresh();
    }

    fn is_composing(&self) -> bool {
        self.snapshot.composing
    }
}
