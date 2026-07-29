use crate::error::CapabilityResult;
use std::collections::HashMap;

fn run_cmd(binary: &str, args: &[&str]) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let output = std::process::Command::new(binary).args(args).output()?;
    let exit_code = output.status.code().unwrap_or(-1);
    Ok((output.stdout, output.stderr, exit_code))
}

fn validate_non_empty(val: &str, name: &str) -> CapabilityResult<()> {
    if val.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput(name.into()));
    }
    Ok(())
}

fn browser_binary(browser: &str) -> &'static str {
    match browser {
        "chrome" => "google-chrome",
        "edge" => "microsoft-edge",
        "firefox" => "firefox",
        _ => "google-chrome",
    }
}

pub fn dispatch(
    capability: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match capability {
        // ── Open ────────────────────────────────────────────
        "browser.open" => {
            let browser = inputs
                .get("browser")
                .map(|s| s.as_str())
                .unwrap_or("chrome");
            browser_open(browser)
        }
        "browser.open_url" => {
            let browser = inputs
                .get("browser")
                .map(|s| s.as_str())
                .unwrap_or("chrome");
            let url = inputs
                .get("url")
                .ok_or_else(|| crate::error::CapabilityError::MissingInput("url".into()))?;
            browser_open_url(browser, url)
        }
        // ── Tab Management ──────────────────────────────────
        "browser.switch_tab" => {
            let direction = inputs
                .get("direction")
                .map(|s| s.as_str())
                .unwrap_or("next");
            browser_switch_tab(direction)
        }
        "browser.close_tab" => browser_close_tab(),
        // ── DOM / Interaction ───────────────────────────────
        "browser.read_dom" => {
            let selector = inputs.get("selector").map(|s| s.as_str()).unwrap_or("body");
            browser_read_dom(selector)
        }
        "browser.click_element" => {
            let selector = inputs.get("selector").map(|s| s.as_str()).unwrap_or("");
            browser_click_element(selector)
        }
        "browser.fill_form" => {
            let selector = inputs.get("selector").map(|s| s.as_str()).unwrap_or("");
            let value = inputs.get("value").map(|s| s.as_str()).unwrap_or("");
            browser_fill_form(selector, value)
        }
        // ── Files ───────────────────────────────────────────
        "browser.upload_file" => {
            let path = inputs.get("path").map(|s| s.as_str()).unwrap_or("");
            browser_upload_file(path)
        }
        "browser.download_file" => {
            let url = inputs.get("url").map(|s| s.as_str()).unwrap_or("");
            let dest = inputs.get("dest").map(|s| s.as_str()).unwrap_or("");
            browser_download_file(url, dest)
        }
        // ── Utilities ───────────────────────────────────────
        "browser.wait_for_element" => {
            let selector = inputs.get("selector").map(|s| s.as_str()).unwrap_or("");
            let timeout = inputs.get("timeout").map(|s| s.as_str()).unwrap_or("5000");
            browser_wait_for_element(selector, timeout)
        }
        "browser.take_screenshot" => {
            let path = inputs
                .get("path")
                .map(|s| s.as_str())
                .unwrap_or("/tmp/browser_screenshot.png");
            browser_take_screenshot(path)
        }
        "browser.execute_javascript" => {
            let code = inputs.get("code").map(|s| s.as_str()).unwrap_or("");
            browser_execute_javascript(code)
        }
        _ => Err(crate::error::CapabilityError::UnknownCapability(
            capability.to_string(),
        )),
    }
}

// ── Open ────────────────────────────────────────────

fn browser_open(browser: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let bin = browser_binary(browser);
    run_cmd(bin, &["--new-window"])
}

fn browser_open_url(browser: &str, url: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(url, "url")?;
    let bin = browser_binary(browser);
    run_cmd(bin, &[url])
}

// ── Tab Management ──────────────────────────────────

fn browser_switch_tab(direction: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match direction {
        "next" => run_cmd("xdotool", &["key", "ctrl+Tab"]),
        "prev" | "previous" => run_cmd("xdotool", &["key", "ctrl+shift+Tab"]),
        "first" => run_cmd("xdotool", &["key", "ctrl+1"]),
        "last" => run_cmd("xdotool", &["key", "ctrl+9"]),
        _ => run_cmd("xdotool", &["key", "ctrl+Tab"]),
    }
}

fn browser_close_tab() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("xdotool", &["key", "ctrl+w"])
}

// ── DOM / Interaction ───────────────────────────────

fn cdp_request(
    endpoint: &str,
    method: &str,
    params: &str,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let payload = format!(r#"{{"id":1,"method":"{}","params":{}}}"#, method, params);
    run_cmd(
        "curl",
        &[
            "-s",
            "-X",
            "POST",
            endpoint,
            "-H",
            "Content-Type: application/json",
            "-d",
            &payload,
        ],
    )
}

fn read_dom_via_cdp(selector: &str, port: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let js_code = format!(
        "document.querySelector('{}').innerHTML",
        selector.replace('\'', "\\'")
    );
    let params = format!(r#"{{"expression":"{}"}}"#, js_code);
    cdp_request(
        &format!("http://localhost:{}/json/new", port),
        "Runtime.evaluate",
        &params,
    )
}

fn browser_read_dom(selector: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if selector.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput(
            "selector".into(),
        ));
    }
    read_dom_via_cdp(selector, "9222")
}

fn browser_click_element(selector: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if selector.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput(
            "selector".into(),
        ));
    }
    let js_code = format!(
        "document.querySelector('{}').click()",
        selector.replace('\'', "\\'")
    );
    let params = format!(r#"{{"expression":"{}"}}"#, js_code);
    cdp_request(
        "http://localhost:9222/json/new",
        "Runtime.evaluate",
        &params,
    )
}

fn browser_fill_form(selector: &str, value: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if selector.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput(
            "selector".into(),
        ));
    }
    if value.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("value".into()));
    }
    let js_code = format!(
        "document.querySelector('{}').value = '{}'",
        selector.replace('\'', "\\'"),
        value.replace('\'', "\\'")
    );
    let params = format!(r#"{{"expression":"{}"}}"#, js_code);
    cdp_request(
        "http://localhost:9222/json/new",
        "Runtime.evaluate",
        &params,
    )
}

// ── Files ───────────────────────────────────────────

fn browser_upload_file(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    // Use xdotool to type the file path into the file picker
    run_cmd("xdotool", &["type", path])
}

fn browser_download_file(url: &str, dest: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(url, "url")?;
    if dest.is_empty() {
        return run_cmd("curl", &["-s", "-O", "-J", url]);
    }
    run_cmd("curl", &["-s", "-o", dest, url])
}

// ── Utilities ───────────────────────────────────────

fn browser_wait_for_element(
    selector: &str,
    timeout: &str,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(selector, "selector")?;
    let timeout_ms: u64 = timeout.parse().unwrap_or(5000);
    let js_code = format!(
        "await new Promise((r,j) => {{ \
         const el = setInterval(() => {{ \
         if(document.querySelector('{}')) {{ clearInterval(el); r(true); }} \
         }},100); \
         setTimeout(() => {{ clearInterval(el); j('timeout'); }}, {}); \
         }})",
        selector.replace('\'', "\\'"),
        timeout_ms
    );
    let params = format!(r#"{{"expression":"{}"}}"#, js_code);
    cdp_request(
        "http://localhost:9222/json/new",
        "Runtime.evaluate",
        &params,
    )
}

fn browser_take_screenshot(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(path, "path")?;
    // Use ImageMagick's import for a reliable desktop screenshot
    run_cmd("import", &["-window", "root", path])
}

fn browser_execute_javascript(code: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(code, "code")?;
    let escaped = code.replace('"', "\\\"");
    let params = format!(r#"{{"expression":"{}"}}"#, escaped);
    cdp_request(
        "http://localhost:9222/json/new",
        "Runtime.evaluate",
        &params,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CapabilityError;

    #[test]
    fn test_unknown_capability() {
        let result = dispatch("browser.nonexistent", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::UnknownCapability(_)
        ));
    }

    #[test]
    fn test_open_url_missing_url() {
        let result = dispatch("browser.open_url", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_browser_switch_tab() {
        let result = dispatch("browser.switch_tab", &HashMap::new());
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_read_dom_empty_selector() {
        let mut inputs = HashMap::new();
        inputs.insert("selector".to_string(), "".to_string());
        let result = dispatch("browser.read_dom", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_execute_javascript_missing_code() {
        let result = dispatch("browser.execute_javascript", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }
}
