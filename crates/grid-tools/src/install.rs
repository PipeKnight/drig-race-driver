//! Finding the purchased installation and reading its Windows-cased paths on
//! case-sensitive filesystems (docs/PLATFORMS_AND_INPUT_RESEARCH.md 1.4, P5).
//!
//! Discovery reads Steam's `steamapps/libraryfolders.vdf` and the GRID app
//! manifest (`appmanifest_12750.acf`, `installdir` "Grid"). Nothing is written.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Steam app id of Race Driver: GRID (from the owner's `appmanifest_12750.acf`).
pub const STEAM_APP_ID: u32 = 12750;
/// A file every supported installation has; used to accept a candidate folder.
pub const MARKER: &str = "cars/lac/lac.ned";

/// Case-insensitive view of one installation directory. Data files name each
/// other with Windows casing; on Linux and Android the real names can differ.
/// The index is built once from a directory listing.
pub struct FileIndex {
    root: PathBuf,
    names: HashMap<String, PathBuf>,
}

impl FileIndex {
    /// Lists every file under `root` (no symlink following, depth 32).
    pub fn build(root: &Path) -> std::io::Result<Self> {
        let mut names = HashMap::new();
        let mut stack = vec![(PathBuf::new(), 0)];
        while let Some((relative, depth)) = stack.pop() {
            for entry in std::fs::read_dir(root.join(&relative))? {
                let entry = entry?;
                let kind = entry.file_type()?;
                let path = relative.join(entry.file_name());
                if kind.is_dir() && depth < 32 {
                    stack.push((path.clone(), depth + 1));
                }
                if kind.is_dir() || kind.is_file() {
                    // The first spelling wins if two names differ only in case.
                    names.entry(key(&path.to_string_lossy())).or_insert(path);
                }
            }
        }
        Ok(Self {
            root: root.to_path_buf(),
            names,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Number of indexed files and directories.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The real path of `relative` (`/` or `\` separated, any casing), or None.
    pub fn find(&self, relative: &str) -> Option<PathBuf> {
        self.names.get(&key(relative)).map(|p| self.root.join(p))
    }

    /// The exact path when it exists, else the indexed spelling, else the
    /// exact path (so the caller's open reports the missing file).
    pub fn resolve(&self, relative: &str) -> PathBuf {
        let exact = self.root.join(relative);
        if exact.exists() {
            return exact;
        }
        self.find(relative).unwrap_or(exact)
    }
}

/// `root/relative` for a name the code derives (for example
/// `interiors/{stem}/int_{stem}_anims.pssg`) when the shipped file spells it
/// differently: the exact path when it exists, else the path whose components
/// match ignoring ASCII case, else the exact path (so the caller's open or
/// `is_file` reports it missing). Windows and macOS find every spelling
/// anyway; Linux and Android storage do not, where the Viper's
/// `int_VIP_anims.pssg` was missed and its cockpit wheel stood still (F921).
/// Only a miss lists directories, one per path component.
pub fn resolve_case(root: &Path, relative: &str) -> PathBuf {
    let exact = root.join(relative);
    if exact.exists() {
        return exact;
    }
    walk_case(root, relative).unwrap_or(exact)
}

/// The on-disk spelling of `root/relative`, matching each component exactly
/// first and then ignoring ASCII case; None when a component has no match.
fn walk_case(root: &Path, relative: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for part in relative.split(['/', '\\']).filter(|p| !p.is_empty() && *p != ".") {
        let names: Vec<_> = std::fs::read_dir(&path).ok()?.flatten().map(|e| e.file_name()).collect();
        let name = names
            .iter()
            .find(|n| n.to_str() == Some(part))
            .or_else(|| names.iter().find(|n| n.to_str().is_some_and(|n| n.eq_ignore_ascii_case(part))))?;
        path.push(name);
    }
    Some(path)
}

fn key(relative: &str) -> String {
    relative
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase()
}

/// Library folders listed in `libraryfolders.vdf` (both the current
/// `"N" { "path" "..." }` form and the old `"N" "path"` form).
pub fn library_folders(vdf: &str) -> Vec<PathBuf> {
    let tokens = tokens(vdf);
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Open => depth += 1,
            Token::Close => depth = depth.saturating_sub(1),
            Token::Text(key) => {
                if let Some(Token::Text(value)) = tokens.get(i + 1) {
                    let new_form = depth == 2 && key.eq_ignore_ascii_case("path");
                    let old_form = depth == 1 && key.bytes().all(|b| b.is_ascii_digit());
                    if new_form || old_form {
                        out.push(PathBuf::from(value));
                    }
                    i += 1;
                }
            }
        }
        i += 1;
    }
    out
}

/// `installdir` of an app manifest (`.acf`), if present.
pub fn install_dir(acf: &str) -> Option<String> {
    let tokens = tokens(acf);
    tokens.windows(2).find_map(|w| match w {
        [Token::Text(k), Token::Text(v)] if k.eq_ignore_ascii_case("installdir") => Some(v.clone()),
        _ => None,
    })
}

enum Token {
    Text(String),
    Open,
    Close,
}

/// Valve KeyValues text: quoted strings with `\\` and `\"` escapes, braces,
/// `//` comments. Unquoted words are accepted as text.
fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => {}
                        },
                        _ => s.push(c),
                    }
                }
                out.push(Token::Text(s));
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {}
            c => {
                let mut s = String::from(c);
                while let Some(&n) = chars.peek() {
                    if n.is_whitespace() || n == '"' || n == '{' || n == '}' {
                        break;
                    }
                    s.push(n);
                    chars.next();
                }
                out.push(Token::Text(s));
            }
        }
    }
    out
}

/// Steam client folders to try, most likely first, for this platform.
pub fn steam_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let env = |name: &str| std::env::var_os(name).map(PathBuf::from);
    if cfg!(windows) {
        for name in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(p) = env(name) {
                out.push(p.join("Steam"));
            }
        }
        out.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    } else if let Some(home) = env("HOME") {
        if cfg!(target_os = "macos") {
            out.push(home.join("Library/Application Support/Steam"));
        } else {
            out.push(home.join(".steam/steam"));
            out.push(home.join(".local/share/Steam"));
            out.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
            // The Windows Steam client run under Wine (Lutris, Bottles, plain wine):
            // the game sits inside the prefix. Native Steam with Proton installs it in
            // the ordinary library above; its compatdata prefix holds only saves.
            let prefix = env("WINEPREFIX").unwrap_or_else(|| home.join(".wine"));
            for programs in ["Program Files (x86)", "Program Files"] {
                out.push(prefix.join("drive_c").join(programs).join("Steam"));
            }
        }
    }
    out
}

/// Whether `dir` looks like a GRID installation (marker file, any casing).
pub fn is_install(dir: &Path) -> bool {
    dir.join(MARKER).is_file()
        || FileIndex::build(dir).is_ok_and(|index| index.find(MARKER).is_some_and(|p| p.is_file()))
}

/// The GRID folder in the first Steam library that has it, searching the
/// given Steam client folders and every library their `libraryfolders.vdf`
/// lists. Returns the candidates tried when nothing is found.
pub fn discover(steam_roots: &[PathBuf]) -> Result<PathBuf, Vec<PathBuf>> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for steam in steam_roots {
        let vdf = steam.join("steamapps").join("libraryfolders.vdf");
        if let Ok(text) = std::fs::read_to_string(&vdf) {
            libraries.extend(library_folders(&text));
        }
        libraries.push(steam.clone());
    }
    let mut tried = Vec::new();
    for library in libraries {
        let apps = library.join("steamapps");
        let dir = std::fs::read_to_string(apps.join(format!("appmanifest_{STEAM_APP_ID}.acf")))
            .ok()
            .and_then(|acf| install_dir(&acf))
            .unwrap_or_else(|| "Grid".into());
        let candidate = apps.join("common").join(dir);
        if tried.contains(&candidate) {
            continue;
        }
        if is_install(&candidate) {
            return Ok(candidate);
        }
        tried.push(candidate);
    }
    Err(tried)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_folders_reads_both_vdf_forms() {
        let current = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps" { "12750" "123" }
	}
	"1" { "path" "D:\\Games\\Steam Library" "apps" { } }
}"#;
        assert_eq!(
            library_folders(current),
            [
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\Games\Steam Library")
            ]
        );
        let old = "\"LibraryFolders\" { \"TimeNextStatsReport\" \"1\" \"1\" \"/home/u/steam lib\" // note\n }";
        assert_eq!(library_folders(old), [PathBuf::from("/home/u/steam lib")]);
    }

    #[test]
    fn install_dir_reads_the_manifest() {
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"12750\"\n\t\"name\"\t\t\"GRID\"\n\t\"installdir\"\t\t\"Grid\"\n}";
        assert_eq!(install_dir(acf).as_deref(), Some("Grid"));
        assert_eq!(install_dir("\"AppState\" { }"), None);
    }

    #[test]
    fn index_resolves_other_casing_and_discovery_follows_libraries() {
        let base = std::env::temp_dir().join(format!("grid-install-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let steam = base.join("Steam");
        let library = base.join("Lib Two");
        let grid = library.join("steamapps/common/GRID_dir");
        std::fs::create_dir_all(grid.join("Cars/LAC")).unwrap();
        std::fs::write(grid.join("Cars/LAC/Lac.NED"), b"x").unwrap();
        std::fs::create_dir_all(steam.join("steamapps")).unwrap();
        let escaped = library.to_string_lossy().replace('\\', "\\\\");
        std::fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!("\"libraryfolders\" {{ \"0\" {{ \"path\" \"{escaped}\" }} }}"),
        )
        .unwrap();
        std::fs::write(
            library.join("steamapps/appmanifest_12750.acf"),
            "\"AppState\" { \"installdir\" \"GRID_dir\" }",
        )
        .unwrap();

        let index = FileIndex::build(&grid).unwrap();
        assert_eq!(index.len(), 3);
        let found = index.find("cars\\lac\\lac.ned").unwrap();
        assert!(found.ends_with("Cars/LAC/Lac.NED"));
        assert!(index.resolve("CARS/lac/LAC.ned").is_file());
        assert_eq!(index.find("cars/lac/missing.ned"), None);
        assert_eq!(discover(std::slice::from_ref(&steam)), Ok(grid.clone()));
        let empty = base.join("NoSteam");
        assert!(discover(&[empty]).is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// A derived lowercase name finds the shipped mixed-case file on every
    /// filesystem (the walk itself is checked, since Windows would find the
    /// exact path anyway).
    #[test]
    fn resolve_case_finds_the_shipped_spelling() {
        let base = std::env::temp_dir().join(format!("grid-resolve-case-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("Interiors/vip")).unwrap();
        std::fs::write(base.join("Interiors/vip/int_VIP_anims.pssg"), b"x").unwrap();
        let walked = walk_case(&base, "interiors/vip/int_vip_anims.pssg").unwrap();
        assert!(walked.ends_with("Interiors/vip/int_VIP_anims.pssg"), "{walked:?}");
        assert!(resolve_case(&base, "interiors/vip/int_vip_anims.pssg").is_file());
        assert!(resolve_case(&base, "INTERIORS\\VIP\\INT_VIP_ANIMS.PSSG").is_file());
        assert_eq!(walk_case(&base, "interiors/vip/int_vip_driver_anims.pssg"), None);
        let missing = resolve_case(&base, "interiors/sal/int_sal_anims.pssg");
        assert_eq!(missing, base.join("interiors/sal/int_sal_anims.pssg"));
        assert!(!missing.exists());
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Every interior's derived animation names reach the shipped files (the
    /// Viper and Saleen spell them `int_VIP_*` and `int_Sal_*`, F921).
    #[test]
    #[ignore = "needs the purchased installation"]
    fn derived_interior_names_resolve_in_the_installation() {
        let root = PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let mut mixed = 0;
        for entry in std::fs::read_dir(root.join("interiors")).unwrap().flatten() {
            let stem = entry.file_name().to_string_lossy().into_owned();
            for suffix in ["", "_anims", "_driver_anims"] {
                let relative = format!("interiors/{stem}/int_{stem}{suffix}.pssg");
                let Some(found) = walk_case(&root, &relative) else { continue };
                assert!(found.is_file(), "{relative}");
                mixed += usize::from(!found.ends_with(&relative));
            }
        }
        // int_VIP_anims, int_VIP_driver_anims, int_Sal_anims, int_Sal_driver_anims.
        assert!(mixed >= 4, "only {mixed} mixed-case interior files");
    }

    /// The owner's install is found from the default Steam folders.
    #[test]
    #[ignore = "needs the purchased installation"]
    fn discovers_the_installed_game() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let found = discover(&steam_roots()).expect("discovery");
        assert_eq!(
            found.canonicalize().unwrap(),
            PathBuf::from(root).canonicalize().unwrap()
        );
        let index = FileIndex::build(&found).unwrap();
        assert!(index.find("CARS/LAC/LAC.NED").is_some());
    }
}
