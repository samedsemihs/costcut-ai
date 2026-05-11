use std::fs;
use std::io::Write;
use std::path::PathBuf;

const BLOCK_START: &str = "# ---- costcut shell block (do not edit manually) ----";
const BLOCK_END: &str = "# ---- end costcut shell block ----";

/// Detect the user's primary shell rc file.
pub fn find_rc_file() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    for candidate in &[".zshrc", ".bashrc", ".bash_profile", ".profile"] {
        let path = home.join(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    // Default to .zshrc or .bashrc depending on shell
    if std::env::var("ZSH_VERSION").is_ok() || std::env::var("SHELL").map_or(false, |s| s.contains("zsh")) {
        Some(home.join(".zshrc"))
    } else {
        Some(home.join(".bashrc"))
    }
}

/// Generate the shell wrapper functions as a string.
pub fn generate_wrapper_block(witcher_binary: &str, interactive_model: Option<&str>) -> String {
    let interactive_model = interactive_model.unwrap_or("");

    format!(
        r#"{BLOCK_START}
# costcut: intelligent model switching for Claude Code, OpenCode, and Pi.
# Analyzes each prompt and picks the best model (Anthropic or DeepSeek).

costcut_exec() {{
    _W_TOOL="$1"
    shift
    _W_PROMPT="$*"

    if [ -z "$_W_PROMPT" ]; then
        command "$_W_TOOL" "$@"
        return $?
    fi

    COSTCUT_BIN="{witcher_binary}"
    if ! command -v "$COSTCUT_BIN" >/dev/null 2>&1; then
        COSTCUT_BIN="costcut"
    fi

    "$COSTCUT_BIN" exec --tool "$_W_TOOL" "$_W_PROMPT"
    return $?
}}

# -- claude (Claude Code) wrapper --
claude() {{
    local _W_ARGS=()
    local _W_PROMPT=""
    local _W_HAS_PROMPT=false

    while [ $# -gt 0 ]; do
        case "$1" in
            -p|--prompt)
                _W_HAS_PROMPT=true
                shift
                if [ $# -gt 0 ]; then
                    _W_PROMPT="$1"
                    shift
                fi
                ;;
            -h|--help|--version|update|mcp)
                command claude "$@"
                return $?
                ;;
            *)
                _W_ARGS+=("$1")
                shift
                ;;
        esac
    done

    if $_W_HAS_PROMPT && [ -n "$_W_PROMPT" ]; then
        costcut_exec claude "$_W_PROMPT"{interactive_var}
    elif [ ${{#_W_ARGS[@]}} -gt 0 ]; then
        command claude "${{_W_ARGS[@]}}"
    else
        command claude
    fi
    return $?
}}

# -- opencode wrapper --
opencode() {{
    if [ $# -eq 0 ]; then
        command opencode
        return $?
    fi

    case "$1" in
        -h|--help|--version)
            command opencode "$@"
            return $?
            ;;
        *)
            costcut_exec opencode "$@"
            return $?
            ;;
    esac
}}

# -- pi wrapper --
pi() {{
    if [ $# -eq 0 ]; then
        command pi
        return $?
    fi

    case "$1" in
        -h|--help|--version)
            command pi "$@"
            return $?
            ;;
        *)
            costcut_exec pi "$@"
            return $?
            ;;
    esac
}}

{BLOCK_END}
"#,
        witcher_binary = witcher_binary,
        interactive_var = if interactive_model.is_empty() {
            String::new()
        } else {
            format!(" {}", interactive_model)
        },
    )
}

/// Check if the costcut block is already present in the rc file.
pub fn has_costcut_block(rc_path: &PathBuf) -> bool {
    if let Ok(content) = fs::read_to_string(rc_path) {
        content.contains(BLOCK_START) && content.contains(BLOCK_END)
    } else {
        false
    }
}

/// Inject the costcut shell block into the rc file.
/// Creates a backup of the original file.
pub fn inject_wrappers(rc_path: &PathBuf, witcher_binary: &str, interactive_model: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let block = generate_wrapper_block(witcher_binary, interactive_model);

    if has_costcut_block(rc_path) {
        // Update existing block
        let content = fs::read_to_string(rc_path)?;
        let start_idx = content.find(BLOCK_START).unwrap();
        let end_idx = content.rfind(BLOCK_END).unwrap() + BLOCK_END.len();

        let mut new_content = String::with_capacity(content.len());
        new_content.push_str(&content[..start_idx]);
        new_content.push_str(&block);
        new_content.push_str(&content[end_idx..]);

        // Backup
        let backup = rc_path.with_extension("costcut.bak");
        fs::write(&backup, &content)?;

        fs::write(rc_path, new_content)?;
        return Ok(());
    }

    // Create backup
    let backup = rc_path.with_extension("costcut.bak");
    if rc_path.exists() {
        fs::copy(rc_path, &backup)?;
    }

    // Append block to file
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rc_path)?;

    // Ensure there's a newline before the block
    if rc_path.exists() {
        let content = fs::read_to_string(rc_path)?;
        if !content.ends_with('\n') {
            writeln!(file)?;
        }
    }

    writeln!(file)?;
    writeln!(file, "{}", block)?;

    Ok(())
}

/// Remove the costcut shell block from the rc file.
pub fn remove_wrappers(rc_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if !has_costcut_block(rc_path) {
        return Ok(());
    }

    let content = fs::read_to_string(rc_path)?;

    let start_idx = content.find(BLOCK_START).unwrap();
    let end_idx = content.rfind(BLOCK_END).unwrap() + BLOCK_END.len();

    // Also consume trailing newlines after the block
    let mut trim_end = end_idx;
    let after = &content[trim_end..];
    for ch in after.chars() {
        if ch == '\n' || ch == '\r' {
            trim_end += ch.len_utf8();
        } else {
            break;
        }
    }

    // Consume leading newlines before the block (if block was alone)
    let mut trim_start = start_idx;
    if trim_start > 0 {
        let before_chars: Vec<char> = content[..trim_start].chars().collect();
        let mut i = before_chars.len();
        while i > 0 {
            i -= 1;
            let ch = before_chars[i];
            if ch == '\n' || ch == '\r' {
                // consume one newline before the block
                if i == before_chars.len() - 1 {
                    trim_start -= ch.len_utf8();
                } else if i == before_chars.len() - 2 {
                    trim_start -= ch.len_utf8();
                }
            } else {
                break;
            }
        }
    }

    let mut new_content = String::with_capacity(content.len());
    new_content.push_str(&content[..trim_start]);
    new_content.push_str(&content[trim_end..]);

    // Remove trailing blank lines at end of file
    let new_content = new_content.trim_end().to_string();

    // Backup
    let backup = rc_path.with_extension("costcut.bak");
    fs::write(&backup, &content)?;

    fs::write(rc_path, new_content + "\n")?;

    Ok(())
}
