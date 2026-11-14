use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub title: String,
    pub exe_path: String,
}

const EXCLUDED: [&str; 3] = ["setup", "redist", "updater"];

/// True for installers, uninstallers, redistributables, crash handlers and updaters.
pub fn is_excluded_exe(file_name: &str) -> bool {
    let n = file_name.to_lowercase();
    n.starts_with("unins") || n.starts_with("install")
        || EXCLUDED.iter().any(|p| n.contains(p))
        || (n.contains("crash") && ["handler", "report", "pad"].iter().any(|p| n.contains(p)))
}

fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase()
}

fn inside(path: &str, known: &str) -> bool {
    path == known || path.strip_prefix(known).is_some_and(|r| r.starts_with('/'))
}

/// Largest non-excluded `.exe` in `dir` or up to `depth` directory levels below it (called with 2).
fn best_exe(dir: &Path, depth: u8) -> Option<(u64, PathBuf)> {
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        let found = if meta.is_dir() {
            if depth == 0 {
                continue;
            }
            best_exe(&path, depth - 1)
        } else {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_exe = name.to_lowercase().ends_with(".exe");
            if is_exe && !is_excluded_exe(&name) { Some((meta.len(), path)) } else { None }
        };
        if let Some(f) = found {
            if best.as_ref().map_or(true, |b| f.0 > b.0) {
                best = Some(f);
            }
        }
    }
    best
}

pub fn candidates(folders: &[PathBuf], known: &[PathBuf]) -> Vec<Candidate> {
    let known: Vec<String> = known.iter().map(|k| norm(k)).collect();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for folder in folders {
        let Ok(rd) = std::fs::read_dir(folder) else { continue };
        let mut subs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        subs.sort();
        for sub in subs {
            let s = norm(&sub);
            if known.iter().any(|k| inside(&s, k)) {
                continue;
            }
            let Some((_, exe)) = best_exe(&sub, 2) else { continue };
            let e = norm(&exe);
            if known.iter().any(|k| inside(&e, k)) {
                continue;
            }
            if !seen.insert(e) {
                continue;
            }
            let Some(title) = sub.file_name().map(|n| n.to_string_lossy().to_string()) else { continue };
            out.push(Candidate { title, exe_path: exe.to_string_lossy().to_string() });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(p: &Path, size: usize) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![0u8; size]).unwrap();
    }

    #[test]
    fn excludes_tools_but_not_launchers() {
        for n in ["unins000.exe", "Setup.exe", "vc_redist.x64.exe", "UnityCrashHandler64.exe", "DXSETUP.exe", "GameUpdater.exe", "install.exe"] {
            assert!(is_excluded_exe(n), "{n}");
        }
        for n in ["Hades.exe", "RiotClientLauncher.exe", "Launcher.exe", "CrashBandicoot.exe", "MyUninstaller.exe"] {
            assert!(!is_excluded_exe(n), "{n}");
        }
    }

    #[test]
    fn picks_largest_non_excluded_and_skips_known() {
        let root = std::env::temp_dir().join(format!("hangar-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let games = root.join("Games");
        write(&games.join("Hades/Hades.exe"), 2048);
        write(&games.join("Hades/unins000.exe"), 5120);
        fs::create_dir_all(games.join("Empty")).unwrap();
        write(&games.join("Known/a.exe"), 100);
        let got = candidates(&[games.clone()], &[games.join("Known")]);
        assert_eq!(
            got,
            vec![Candidate { title: "Hades".into(), exe_path: games.join("Hades").join("Hades.exe").to_string_lossy().to_string() }]
        );
        // Known exe path also suppresses its folder.
        let got = candidates(&[games.clone()], &[games.join("Hades").join("Hades.exe")]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title, "Known");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn overlapping_folders_yield_one_candidate() {
        let root = std::env::temp_dir().join(format!("hangar-scan-dedupe-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let games = root.join("Games");
        write(&games.join("Sub/Hades/Hades.exe"), 2048);
        let got = candidates(&[games.clone(), games.join("Sub")], &[]);
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].title, "Sub"); // first folder wins
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_folder_is_empty() {
        assert!(candidates(&[PathBuf::from("/definitely/not/here")], &[]).is_empty());
    }
}
