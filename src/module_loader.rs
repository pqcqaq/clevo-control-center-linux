use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::preferences::UiLanguage;

const REQUIRED_PROC_NODES: [&str; 3] = [
    "/proc/clevo_control_center_led",
    "/proc/clevo_dchu_status",
    "/proc/clevo_dchu_control",
];
const MODULE_VERSION_PROC: &str = "/proc/clevo_control_center_version";
const REQUIRED_MODULE_API_VERSION: u32 = 5;
const MODULE_FILE_NAME: &str = "clevo_control_center.ko";

pub fn ensure_module_loaded_for_gui(language: UiLanguage) -> bool {
    let state = module_state();
    if state == ModuleState::Ready {
        return true;
    }

    if !confirm_load_module(state, language) {
        return false;
    }

    match load_module_with_auth(language) {
        Ok(()) if module_state() == ModuleState::Ready => true,
        Ok(()) => {
            show_error(
                language.pick(
                    "模块加载/更新命令已执行，但模块版本仍不可用或过旧。",
                    "The module command completed, but the required module version is still unavailable.",
                ),
                language,
            );
            false
        }
        Err(err) => {
            let text = match language {
                UiLanguage::SimplifiedChinese => format!("模块加载/更新失败：{err}"),
                UiLanguage::English => format!("Module loading or update failed: {err}"),
            };
            show_error(&text, language);
            false
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModuleState {
    Ready,
    Missing,
    Outdated(Option<u32>),
}

fn module_state() -> ModuleState {
    if !REQUIRED_PROC_NODES
        .iter()
        .all(|path| Path::new(path).exists())
    {
        return ModuleState::Missing;
    }

    match module_api_version() {
        Some(version) if version >= REQUIRED_MODULE_API_VERSION => ModuleState::Ready,
        version => ModuleState::Outdated(version),
    }
}

fn module_api_version() -> Option<u32> {
    fs::read_to_string(MODULE_VERSION_PROC)
        .ok()
        .and_then(|text| parse_module_api_version(&text))
}

fn parse_module_api_version(text: &str) -> Option<u32> {
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("api_version ")
            .and_then(|value| value.trim().parse::<u32>().ok())
    })
}

fn confirm_load_module(state: ModuleState, language: UiLanguage) -> bool {
    let text = match state {
        ModuleState::Ready => return true,
        ModuleState::Missing => language.pick(
            "Clevo 控制中心内核模块未加载。是否立即通过系统认证加载？",
            "The Clevo Control Center kernel module is not loaded. Authenticate to load it now?",
        ).to_owned(),
        ModuleState::Outdated(Some(version)) => match language {
            UiLanguage::SimplifiedChinese => format!(
                "Clevo 控制中心内核模块版本过旧（当前 API {version}，需要 API {REQUIRED_MODULE_API_VERSION}）。是否立即通过系统认证更新？"
            ),
            UiLanguage::English => format!(
                "The kernel module is outdated (API {version}; API {REQUIRED_MODULE_API_VERSION} is required). Authenticate to update it now?"
            ),
        },
        ModuleState::Outdated(None) => language.pick(
            "Clevo 控制中心内核模块版本过旧或无法读取版本。是否立即通过系统认证更新？",
            "The kernel module is outdated or its version cannot be read. Authenticate to update it now?",
        ).to_owned(),
    };
    let text_arg = format!("--text={text}");
    let title = format!(
        "--title={}",
        language.pick("模块需要加载", "Kernel module required")
    );
    let accept = format!("--ok-label={}", language.pick("立即处理", "Continue"));
    let cancel = format!("--cancel-label={}", language.pick("关闭", "Close"));

    match run_zenity(&["--question", &title, &text_arg, &accept, &cancel]) {
        DialogResult::Accepted => return true,
        DialogResult::Rejected => return false,
        DialogResult::Unavailable => {}
    }

    match run_kdialog(&[
        "--yesno",
        &text,
        "--title",
        language.pick("模块需要加载", "Kernel module required"),
    ]) {
        DialogResult::Accepted => true,
        DialogResult::Rejected | DialogResult::Unavailable => {
            eprintln!("{text}");
            false
        }
    }
}

fn show_error(text: &str, language: UiLanguage) {
    let title = format!(
        "--title={}",
        language.pick("模块加载失败", "Module loading failed")
    );
    if matches!(
        run_zenity(&["--error", &title, &format!("--text={text}"),]),
        DialogResult::Accepted | DialogResult::Rejected
    ) {
        return;
    }
    if matches!(
        run_kdialog(&[
            "--error",
            text,
            "--title",
            language.pick("模块加载失败", "Module loading failed"),
        ]),
        DialogResult::Accepted | DialogResult::Rejected
    ) {
        return;
    }
    eprintln!("{text}");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DialogResult {
    Accepted,
    Rejected,
    Unavailable,
}

fn run_zenity(args: &[&str]) -> DialogResult {
    run_dialog("zenity", args)
}

fn run_kdialog(args: &[&str]) -> DialogResult {
    run_dialog("kdialog", args)
}

fn run_dialog(program: &str, args: &[&str]) -> DialogResult {
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .status();

    match status {
        Ok(status) if status.success() => DialogResult::Accepted,
        Ok(_) => DialogResult::Rejected,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => DialogResult::Unavailable,
        Err(_) => DialogResult::Rejected,
    }
}

fn load_module_with_auth(language: UiLanguage) -> Result<(), String> {
    let module_path = module_path_candidate()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let script = r#"
set -eu
PATH=/usr/sbin:/sbin:/usr/bin:/bin
modprobe -r clevo_kbd_led 2>/dev/null || true
modprobe -r clevo_control_center 2>/dev/null || true
rmmod clevo_control_center 2>/dev/null || true
if [ -n "${1:-}" ] && [ -f "$1" ]; then
    insmod "$1"
    exit 0
fi
if modprobe clevo_control_center 2>/dev/null; then
    exit 0
fi
exit 1
"#;

    let output = Command::new("pkexec")
        .arg("sh")
        .arg("-c")
        .arg(script)
        .arg("clevo-control-center-module-loader")
        .arg(module_path)
        .output()
        .map_err(|err| match language {
            UiLanguage::SimplifiedChinese => format!("无法启动 pkexec：{err}"),
            UiLanguage::English => format!("Could not start pkexec: {err}"),
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = format!("{stdout}{stderr}").trim().to_owned();
    if detail.is_empty() {
        Err(match language {
            UiLanguage::SimplifiedChinese => format!("pkexec 返回 {}", output.status),
            UiLanguage::English => format!("pkexec returned {}", output.status),
        })
    } else {
        Err(detail)
    }
}

fn module_path_candidate() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .and_then(|dir| module_path_from_dir(&dir))
        .or_else(|| {
            let exe = std::env::current_exe().ok()?;
            let parent = exe.parent()?;
            parent.ancestors().find_map(module_path_from_dir)
        })
        .or_else(|| module_path_from_dir(Path::new("/usr/lib/clevo-control-center")))
}

fn module_path_from_dir(dir: &Path) -> Option<PathBuf> {
    let candidate = dir.join("module").join(MODULE_FILE_NAME);
    candidate.exists().then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_module_api_version() {
        assert_eq!(parse_module_api_version("api_version 2\n"), Some(2));
        assert_eq!(
            parse_module_api_version("name clevo\napi_version 12\n"),
            Some(12)
        );
        assert_eq!(parse_module_api_version("version 2\n"), None);
        assert_eq!(parse_module_api_version("api_version nope\n"), None);
    }
}
