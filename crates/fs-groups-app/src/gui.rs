//! The window of `fs-groups` (ADR 0063): wiring only.
//!
//! Pattern: Facade consumer — connect to the server (starting it if none answers on the user's own
//! socket), read its catalog of type descriptions, give every data type a surface of the generated Admin frontend
//! and put them in the standard window shell (ADR 0065); open the window or draw one picture.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_admin_frontend::view::{AboutInfo, AdminSurface};
use fs_admin_frontend::window::{TypeView, admin_window};
use fs_admin_frontend::{Backend, Session, SystemClock};
use fs_components_app::{AppError, AppWidget, WindowOptions, launch, parse_size};
use fs_components_fields::{Look, Say};
use fs_components_styles::StyleId;
use fs_groups_core::describe::TYPES;
use fs_interfaces::rpc::default_socket;
use fs_types::TypeRegistry;

use crate::words::{English, TITLE};
use crate::{GroupsBackend, GroupsRemote};

/// The help text.
pub const HELP: &str = "fs-groups: the groups window (without arguments it opens)

  --socket <path>    the groups server to talk to (default: $XDG_RUNTIME_DIR/freesynergy/groups.sock);
                     without it, a server is started if none answers; start it yourself with `fs-groups serve`
  --snapshot <png>   draw a picture instead of opening a window
  --size <WxH>       window or picture size (default 1200x800)";

/// The page the Help entry opens.
pub const HELP_URL: &str =
    "https://github.com/FreeSynergy/fs-documentation/blob/main/docs/en/programs/platform/groups.md";

fn about() -> AboutInfo {
    AboutInfo {
        name: "FreeSynergy.Groups".into(),
        version: format!("v{}", env!("CARGO_PKG_VERSION")),
        origin: "https://github.com/FreeSynergy/fs-groups".into(),
        licence: "MIT OR Apache-2.0".into(),
        repository: "https://github.com/FreeSynergy/fs-groups".into(),
    }
}

/// The window over the groups server behind `remote`: a screen per data type of the catalog the server serves.
///
/// # Errors
/// The text for the person: the catalog cannot be read, or the shell cannot be made.
pub fn app_widget(remote: &Arc<GroupsRemote>) -> Result<AppWidget, String> {
    let catalog = remote.catalog().map_err(|e| format!("{e:?}"))?;
    app_widget_for(remote, &catalog).map_err(|e| e.to_string())
}

/// As [`app_widget`], over a catalog already read.
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn app_widget_for(
    remote: &Arc<GroupsRemote>,
    catalog: &TypeRegistry,
) -> Result<AppWidget, AppError> {
    let say: Arc<dyn Say> = Arc::new(English);
    let views = TYPES
        .iter()
        .filter_map(|name| catalog.get(name).map(|d| (*name, d.clone())))
        .map(|(name, description)| {
            let backend: Arc<dyn Backend> = Arc::new(GroupsBackend::new(Arc::clone(remote), name));
            let session = Session::new(description.clone(), backend, Arc::new(SystemClock::new()));
            TypeView {
                id: name.to_owned(),
                label: description.label.clone(),
                surface: Arc::new(AdminSurface::new(
                    session,
                    Look::new(StyleId::Admin, false),
                    Arc::clone(&say),
                    None,
                )),
            }
        })
        .collect();
    admin_window(TITLE, &say, views, about(), HELP_URL.to_owned())
}

struct Args {
    socket: Option<PathBuf>,
    snapshot: Option<PathBuf>,
    size: (u32, u32),
    help: bool,
}

fn parse(words: &[String]) -> Result<Args, String> {
    let mut out = Args {
        socket: None,
        snapshot: None,
        size: (1200, 800),
        help: false,
    };
    let mut it = words.iter();
    while let Some(word) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match word.as_str() {
            "--socket" => out.socket = Some(PathBuf::from(value("--socket")?)),
            "--snapshot" => out.snapshot = Some(PathBuf::from(value("--snapshot")?)),
            "--size" => out.size = parse_size(&value("--size")?)?,
            "-h" | "--help" => out.help = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(out)
}

/// Open the window. `words` are the words without the program name.
///
/// # Errors
/// The message for the person: bad words, no server, a picture that cannot be drawn.
pub fn run(words: &[String]) -> Result<(), String> {
    let args = parse(words)?;
    if args.help {
        println!("{HELP}");
        return Ok(());
    }
    let socket = args
        .socket
        .clone()
        .or_else(|| default_socket("groups"))
        .ok_or("no socket: give --socket or set XDG_RUNTIME_DIR")?;
    let remote = connect_or_start(&socket, args.socket.is_none())?;
    let app = app_widget(&Arc::new(remote))?;
    let (width, height) = args.size;
    if let Some(file) = &args.snapshot {
        app.picture(width, height, file)
            .map_err(|e| e.to_string())?;
        eprintln!("wrote {}", file.display());
        return Ok(());
    }
    let mut options = WindowOptions::default();
    options.width = width;
    options.height = height;
    launch(options, app).map_err(|e| e.to_string())
}

/// Connect; if none answers on the user's own socket, start `fs-groups serve` (detached, so other
/// windows use it too) and wait for it. A socket given with `--socket` is never answered by starting
/// anything.
fn connect_or_start(socket: &Path, may_start: bool) -> Result<GroupsRemote, String> {
    let first = GroupsRemote::connect(socket);
    if first.is_ok() || !may_start {
        return first;
    }
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut server = std::process::Command::new(me);
    server
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut server, 0);
    server
        .spawn()
        .map_err(|e| format!("could not start the groups server: {e}"))?;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(remote) = GroupsRemote::connect(socket) {
            return Ok(remote);
        }
    }
    first
}
