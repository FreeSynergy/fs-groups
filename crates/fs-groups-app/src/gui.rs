//! The window of `fs-groups` (ADR 0063): wiring only.
//!
//! Pattern: Facade consumer — connect to the server (the version handshake: a server of another version is
//! replaced, none is started; a failure is shown in the window, never a black one), build the workbench, open
//! the window or draw one picture.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_components_app::{
    AppError, AppWidget, Element, Screen, Surface, WindowOptions, launch, parse_size,
};
use fs_components_workbenches::people::{PeopleSource, PeopleWidget};
use fs_interfaces::rpc::{CommandStarter, default_socket, parse_facet};

use crate::words::{TITLE, labels};
use crate::{GroupsAdapter, GroupsRemote};

/// The one facet of this program.
pub const FACET: &str = "groups";

/// The help text.
pub const HELP: &str = "fs-groups: the groups window (without arguments it opens)

  --socket <path>    the groups server to talk to (default: $XDG_RUNTIME_DIR/freesynergy/groups.sock);
                     a server that is not there, or of another version, is replaced by one of this version
  --facet <name>     show one facet only: groups (the only one this program has)
  --snapshot <png>   draw a picture instead of opening a window
  --size <WxH>       window or picture size (default 1200x800)";

struct GroupsScreen(PeopleWidget);

impl Surface for GroupsScreen {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        Box::new(self.0.build(width, height))
    }

    fn styles(&self) -> String {
        self.0.styles()
    }
}

/// The shell around the workbench over `source`.
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn app_widget(source: Arc<dyn PeopleSource>) -> Result<AppWidget, AppError> {
    let widget = PeopleWidget::new(source, labels());
    let screen = Screen::new("groups", TITLE, Arc::new(GroupsScreen(widget)));
    AppWidget::new(TITLE, vec![screen])
}

struct Args {
    facet: Option<String>,
    socket: Option<PathBuf>,
    snapshot: Option<PathBuf>,
    size: (u32, u32),
    help: bool,
}

fn parse(words: &[String]) -> Result<Args, String> {
    let mut out = Args {
        facet: None,
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
            "--facet" => {
                let name = parse_facet(&value("--facet")?)?;
                if name != FACET {
                    return Err(format!("unknown facet {name}; this program has: {FACET}"));
                }
                out.facet = Some(name);
            }
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
    // The typing test reads what is typed into the fields: the registry must be on BEFORE the first field is made, so
    // before the window is built. A debug-only feature, off in a release.
    #[cfg(feature = "test-access")]
    fs_components_widgets::test_access::start_from_env();
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
    let remote =
        GroupsRemote::matching(&socket, Arc::new(starter(&socket, args.socket.is_some())?));
    let adapter = GroupsAdapter::new(Arc::new(remote));
    let app = app_widget(Arc::new(adapter)).map_err(|e| e.to_string())?;
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

/// How this window starts the server when none fits: `fs-groups serve` (detached by `CommandStarter`, so other
/// windows use it too), on the socket this window talks to. What it says when it cannot start it says on its own
/// stderr, which a detached server has not: the person sees the handshake error instead.
fn starter(socket: &Path, own_socket: bool) -> Result<CommandStarter, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut words: Vec<std::ffi::OsString> = vec!["serve".into()];
    if own_socket {
        words.extend(["--socket".into(), socket.as_os_str().to_owned()]);
    }
    Ok(CommandStarter::new(me, words))
}
