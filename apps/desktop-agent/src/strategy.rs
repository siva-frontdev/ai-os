use std::collections::HashMap;

use perception_core::DesktopState;

#[derive(Debug, Clone)]
pub enum ActionStrategy {
    FocusWindow { title: String, partial: bool },
    CloseWindow { title: String },
    OpenUrl { url: String },
    LaunchApp { app: String },
    MouseMove { x: i32, y: i32 },
    MouseClick { button: String },
    MouseMoveClick { x: i32, y: i32, button: String },
    KeyboardType { text: String },
    KeyboardCombo { keys: Vec<String> },
    ClipboardSet { text: String },
    ShellCommand { command: Vec<String> },
    WaitMs { duration_ms: u64 },
    Noop,
}

pub struct StrategySelector;

impl StrategySelector {
    pub fn strategies_for(
        capability_id: &str,
        params: &HashMap<String, String>,
        state: &DesktopState,
    ) -> Vec<ActionStrategy> {
        match capability_id {
            "desktop.window.focus" => {
                let title = params
                    .get("title")
                    .or_else(|| params.get("id"))
                    .cloned()
                    .unwrap_or_default();
                Self::focus_window_strategies(&title, state)
            }
            "desktop.window.close" => {
                let title = params.get("id").cloned().unwrap_or_default();
                Self::close_window_strategies(&title)
            }

            "desktop.app.launch" => {
                let app = params.get("app").cloned().unwrap_or_default();
                Self::launch_app_strategies(&app)
            }

            "input.mouse.move" => {
                let x = params.get("x").and_then(|v| v.parse().ok()).unwrap_or(0);
                let y = params.get("y").and_then(|v| v.parse().ok()).unwrap_or(0);
                Self::mouse_move_strategies(x, y)
            }
            "input.mouse.click" => {
                let button = params.get("button").cloned().unwrap_or_else(|| "1".into());
                Self::mouse_click_strategies(&button)
            }

            "input.keyboard.type" => {
                let text = params.get("text").cloned().unwrap_or_default();
                Self::keyboard_type_strategies(&text)
            }
            "input.keyboard.combo" | "input.keyboard.shortcut" => {
                let raw = params
                    .get("keys")
                    .or_else(|| params.get("shortcut"))
                    .cloned()
                    .unwrap_or_default();
                let keys: Vec<String> = raw.split('+').map(|s| s.trim().to_string()).collect();
                vec![ActionStrategy::KeyboardCombo { keys }]
            }

            "desktop.clipboard.set" => {
                let text = params.get("text").cloned().unwrap_or_default();
                vec![ActionStrategy::ClipboardSet { text }]
            }

            "browser.open_url" | "open_website" => {
                let url = params.get("url").cloned().unwrap_or_default();
                if url.is_empty() {
                    vec![ActionStrategy::Noop]
                } else {
                    vec![ActionStrategy::OpenUrl { url }]
                }
            }
            "search_web" | "search" => {
                let query = params.get("query").cloned().unwrap_or_default();
                if query.is_empty() {
                    vec![ActionStrategy::Noop]
                } else {
                    let encoded: String = query
                        .chars()
                        .map(|c| match c {
                            ' ' => '+',
                            _ => c,
                        })
                        .collect();
                    vec![ActionStrategy::OpenUrl {
                        url: format!("https://google.com/search?q={}", encoded),
                    }]
                }
            }

            "wait" | "delay" | "sleep" => {
                let ms = params.get("ms").and_then(|v| v.parse().ok()).unwrap_or(500);
                vec![ActionStrategy::WaitMs { duration_ms: ms }]
            }

            _ => {
                let cmd: Vec<String> = params
                    .get("command")
                    .map(|c| c.split_whitespace().map(|s| s.to_string()).collect())
                    .unwrap_or_default();
                if !cmd.is_empty() {
                    vec![ActionStrategy::ShellCommand { command: cmd }]
                } else {
                    vec![ActionStrategy::Noop]
                }
            }
        }
    }

    fn focus_window_strategies(title: &str, state: &DesktopState) -> Vec<ActionStrategy> {
        let mut strategies = Vec::new();
        strategies.push(ActionStrategy::FocusWindow {
            title: title.to_string(),
            partial: false,
        });
        if state
            .windows
            .iter()
            .any(|w| w.title.to_lowercase().contains(&title.to_lowercase()))
        {
            strategies.push(ActionStrategy::FocusWindow {
                title: title.to_string(),
                partial: true,
            });
        }
        strategies
    }

    fn close_window_strategies(title: &str) -> Vec<ActionStrategy> {
        vec![
            ActionStrategy::CloseWindow {
                title: title.to_string(),
            },
            ActionStrategy::KeyboardCombo {
                keys: vec!["alt".into(), "F4".into()],
            },
        ]
    }

    fn launch_app_strategies(app: &str) -> Vec<ActionStrategy> {
        vec![ActionStrategy::LaunchApp {
            app: app.to_string(),
        }]
    }

    fn mouse_move_strategies(x: i32, y: i32) -> Vec<ActionStrategy> {
        vec![ActionStrategy::MouseMove { x, y }]
    }

    fn mouse_click_strategies(button: &str) -> Vec<ActionStrategy> {
        vec![ActionStrategy::MouseClick {
            button: button.to_string(),
        }]
    }

    fn keyboard_type_strategies(text: &str) -> Vec<ActionStrategy> {
        vec![ActionStrategy::KeyboardType {
            text: text.to_string(),
        }]
    }
}
