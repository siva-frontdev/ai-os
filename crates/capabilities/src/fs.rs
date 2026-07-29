use crate::error::CapabilityResult;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;

fn run_cmd(binary: &str, args: &[&str]) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let output = std::process::Command::new(binary).args(args).output()?;
    let exit_code = output.status.code().unwrap_or(-1);
    Ok((output.stdout, output.stderr, exit_code))
}

fn val<'a>(inputs: &'a HashMap<String, String>, key: &str) -> CapabilityResult<&'a str> {
    inputs
        .get(key)
        .map(|s| s.as_str())
        .ok_or_else(|| crate::error::CapabilityError::MissingInput(key.into()))
}

fn validate_non_empty(val: &str, name: &str) -> CapabilityResult<()> {
    if val.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput(name.into()));
    }
    Ok(())
}

pub fn dispatch(
    capability: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match capability {
        // ── File Operations ──────────────────────────
        "fs.file.read" => {
            let path = val(inputs, "path")?;
            fs_file_read(path)
        }
        "fs.file.write" => {
            let path = val(inputs, "path")?;
            let content = val(inputs, "content")?;
            fs_file_write(path, content)
        }
        "fs.file.copy" => {
            let src = val(inputs, "source")?;
            let dest = val(inputs, "destination")?;
            fs_file_copy(src, dest)
        }
        "fs.file.move" => {
            let src = val(inputs, "source")?;
            let dest = val(inputs, "destination")?;
            fs_file_move(src, dest)
        }
        "fs.file.delete" => {
            let path = val(inputs, "path")?;
            fs_file_delete(path)
        }
        "fs.file.append" => {
            let path = val(inputs, "path")?;
            let content = val(inputs, "content")?;
            fs_file_append(path, content)
        }
        "fs.file.touch" => {
            let path = val(inputs, "path")?;
            fs_file_touch(path)
        }
        "fs.file.stat" => {
            let path = val(inputs, "path")?;
            fs_file_stat(path)
        }
        // ── Directory Operations ──────────────────────
        "fs.dir.list" => {
            let path = val(inputs, "path")?;
            fs_dir_list(path)
        }
        "fs.dir.create" => {
            let path = val(inputs, "path")?;
            fs_dir_create(path)
        }
        "fs.dir.delete" => {
            let path = val(inputs, "path")?;
            fs_dir_delete(path)
        }
        "fs.dir.tree" => {
            let path = val(inputs, "path")?;
            fs_dir_tree(path)
        }
        // ── Search ──────────────────────────────────
        "fs.search.name" => {
            let root = val(inputs, "root")?;
            let pattern = val(inputs, "pattern")?;
            fs_search_name(root, pattern)
        }
        "fs.search.content" => {
            let root = val(inputs, "root")?;
            let text = val(inputs, "text")?;
            fs_search_content(root, text)
        }
        // ── Archive ────────────────────────────────
        "fs.archive.create" => {
            let archive = val(inputs, "archive")?;
            let sources = val(inputs, "sources")?;
            let format = inputs.get("format").map(|s| s.as_str()).unwrap_or("zip");
            fs_archive_create(archive, sources, format)
        }
        "fs.archive.extract" => {
            let archive = val(inputs, "archive")?;
            let dest = val(inputs, "destination")?;
            fs_archive_extract(archive, dest)
        }
        // ── Watch ──────────────────────────────────
        "fs.watch" => {
            let path = val(inputs, "path")?;
            fs_watch(path)
        }
        _ => Err(crate::error::CapabilityError::UnknownCapability(
            capability.to_string(),
        )),
    }
}

// ── File Operations ──────────────────────────

fn fs_file_read(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    let content = std::fs::read(path)?;
    Ok((content, Vec::new(), 0))
}

fn fs_file_write(path: &str, content: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    std::fs::write(path, content)?;
    Ok((b"written".to_vec(), Vec::new(), 0))
}

fn fs_file_copy(source: &str, dest: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(source, "source")?;
    validate_non_empty(dest, "destination")?;
    std::fs::copy(source, dest)?;
    Ok((b"copied".to_vec(), Vec::new(), 0))
}

fn fs_file_move(source: &str, dest: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(source, "source")?;
    validate_non_empty(dest, "destination")?;
    std::fs::rename(source, dest)?;
    Ok((b"moved".to_vec(), Vec::new(), 0))
}

fn fs_file_delete(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    if std::fs::metadata(path)?.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok((b"deleted".to_vec(), Vec::new(), 0))
}

fn fs_file_append(path: &str, content: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)?;
    file.write_all(content.as_bytes())?;
    Ok((b"appended".to_vec(), Vec::new(), 0))
}

fn fs_file_touch(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(path);
    match file {
        Ok(_) => Ok((b"touched".to_vec(), Vec::new(), 0)),
        Err(e) => Err(crate::error::CapabilityError::ExecutionFailed(
            e.to_string(),
        )),
    }
}

fn fs_file_stat(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    let meta = std::fs::metadata(path)?;
    let info = format!(
        "size:{}\nis_dir:{}\nis_file:{}\npermissions:{:o}\nmodified:{:?}",
        meta.len(),
        meta.is_dir(),
        meta.is_file(),
        meta.permissions().mode(),
        meta.modified(),
    );
    Ok((info.into_bytes(), Vec::new(), 0))
}

// ── Directory Operations ──────────────────────

fn fs_dir_list(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    let mut entries = std::fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    entries.sort();
    Ok((entries.join("\n").into_bytes(), Vec::new(), 0))
}

fn fs_dir_create(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    std::fs::create_dir_all(path)?;
    Ok((b"created".to_vec(), Vec::new(), 0))
}

fn fs_dir_delete(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    std::fs::remove_dir_all(path)?;
    Ok((b"deleted".to_vec(), Vec::new(), 0))
}

fn fs_dir_tree(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("find", &[path, "-print"])
}

// ── Search ──────────────────────────────────

fn fs_search_name(root: &str, pattern: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(root, "root")?;
    validate_non_empty(pattern, "pattern")?;
    run_cmd("find", &[root, "-name", pattern])
}

fn fs_search_content(root: &str, text: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(root, "root")?;
    validate_non_empty(text, "text")?;
    run_cmd("grep", &["-r", "-l", text, root])
}

// ── Archive ────────────────────────────────

fn fs_archive_create(
    archive: &str,
    sources: &str,
    format: &str,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(archive, "archive")?;
    validate_non_empty(sources, "sources")?;
    let parts: Vec<&str> = sources.split(' ').collect();
    match format {
        "zip" => {
            let mut args = vec!["-r", archive];
            args.extend(&parts);
            run_cmd("zip", &args)
        }
        "tar.gz" | "tgz" => {
            let mut args = vec!["-czf", archive];
            args.extend(&parts);
            run_cmd("tar", &args)
        }
        "tar.bz2" => {
            let mut args = vec!["-cjf", archive];
            args.extend(&parts);
            run_cmd("tar", &args)
        }
        "tar.xz" => {
            let mut args = vec!["-cJf", archive];
            args.extend(&parts);
            run_cmd("tar", &args)
        }
        "7z" => {
            let mut args = vec!["a", archive];
            args.extend(&parts);
            run_cmd("7z", &args)
        }
        _ => Err(crate::error::CapabilityError::InvalidInput(format!(
            "unsupported archive format: {}",
            format
        ))),
    }
}

fn fs_archive_extract(archive: &str, dest: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(archive, "archive")?;
    validate_non_empty(dest, "destination")?;
    if archive.ends_with(".zip") {
        run_cmd("unzip", &["-o", archive, "-d", dest])
    } else if archive.ends_with(".tar.gz") || archive.ends_with(".tgz") {
        run_cmd("tar", &["-xzf", archive, "-C", dest])
    } else if archive.ends_with(".tar.bz2") {
        run_cmd("tar", &["-xjf", archive, "-C", dest])
    } else if archive.ends_with(".tar.xz") {
        run_cmd("tar", &["-xJf", archive, "-C", dest])
    } else if archive.ends_with(".7z") {
        run_cmd("7z", &["x", archive, format!("-o{}", dest).as_str()])
    } else {
        // fallback: try tar
        run_cmd("tar", &["-xf", archive, "-C", dest])
    }
}

// ── Watch ──────────────────────────────────

fn fs_watch(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    let meta = std::fs::metadata(path)?;
    let file_type = if meta.is_dir() { "directory" } else { "file" };
    let output = format!(
        "watching {}: {}\nnote: persistent watch requires inotify-based implementation",
        file_type, path
    );
    Ok((output.into_bytes(), Vec::new(), 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CapabilityError;

    #[test]
    fn test_unknown_capability() {
        let result = dispatch("fs.nonexistent", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::UnknownCapability(_)
        ));
    }

    #[test]
    fn test_file_read_missing_path() {
        let result = dispatch("fs.file.read", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_file_write_missing_content() {
        let mut inputs = HashMap::new();
        inputs.insert("path".to_string(), "/tmp/test.txt".to_string());
        let result = dispatch("fs.file.write", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_file_copy_missing_dest() {
        let mut inputs = HashMap::new();
        inputs.insert("source".to_string(), "/tmp/a.txt".to_string());
        let result = dispatch("fs.file.copy", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_dir_list_missing_path() {
        let result = dispatch("fs.dir.list", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_search_name_missing_pattern() {
        let mut inputs = HashMap::new();
        inputs.insert("root".to_string(), "/".to_string());
        let result = dispatch("fs.search.name", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_archive_create_missing_sources() {
        let mut inputs = HashMap::new();
        inputs.insert("archive".to_string(), "/tmp/test.zip".to_string());
        let result = dispatch("fs.archive.create", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_archive_create_unsupported_format() {
        let mut inputs = HashMap::new();
        inputs.insert("archive".to_string(), "/tmp/test.xyz".to_string());
        inputs.insert("sources".to_string(), "file.txt".to_string());
        inputs.insert("format".to_string(), "xyz".to_string());
        let result = dispatch("fs.archive.create", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::InvalidInput(_)
        ));
    }

    #[test]
    fn test_archive_extract_missing_dest() {
        let mut inputs = HashMap::new();
        inputs.insert("archive".to_string(), "/tmp/test.zip".to_string());
        let result = dispatch("fs.archive.extract", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_watch_missing_path() {
        let result = dispatch("fs.watch", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_fs_file_stat_success() {
        let result = fs_file_stat("/tmp");
        assert!(result.is_ok());
        let (stdout, _, _) = result.unwrap();
        let output = String::from_utf8_lossy(&stdout);
        assert!(output.contains("is_dir:true"));
    }

    #[test]
    fn test_fs_file_touch_success() {
        let path = "/tmp/_test_touch.txt";
        let _ = std::fs::remove_file(path);
        let result = fs_file_touch(path);
        assert!(result.is_ok());
        assert!(std::path::Path::new(path).exists());
        let _ = std::fs::remove_file(path);
    }
}
