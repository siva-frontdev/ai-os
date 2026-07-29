use std::collections::HashMap;

use intelligence_coordinator::world_understanding::{
    StateChange, StructuredWorldUpdate, WorldEntity, WorldRelationship,
};
use memory_core::wm::Entity;

/// One day in a simulation scenario.
#[derive(Debug, Clone)]
pub struct ScenarioDay {
    /// User's observation (None = no input, just tick).
    pub observation: Option<String>,
    /// The structured understanding the "AI" extracts from the observation.
    /// If observation is None, this is ignored (no cycle() called).
    pub understanding: Option<StructuredWorldUpdate>,
}

impl ScenarioDay {
    pub fn new(observation: Option<&str>, understanding: Option<StructuredWorldUpdate>) -> Self {
        Self {
            observation: observation.map(|s| s.to_string()),
            understanding,
        }
    }

    /// A day with only a tick() — no user interaction.
    pub fn silent() -> Self {
        Self {
            observation: None,
            understanding: None,
        }
    }
}

/// A named simulation scenario with initial world model state and a day schedule.
#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: &'static str,
    pub description: &'static str,
    pub initial_entities: Vec<Entity>,
    pub days: Vec<ScenarioDay>,
}

// ── Helper constructors ──────────────────────────────────────

fn make_entity(name: &str, entity_type: &str, importance: f64, confidence: f64) -> WorldEntity {
    WorldEntity {
        name: name.into(),
        entity_type: entity_type.into(),
        properties: HashMap::new(),
        confidence,
        importance,
    }
}

fn make_update(
    entities: Vec<WorldEntity>,
    relationships: Vec<WorldRelationship>,
    state_changes: Vec<StateChange>,
    new_observations: Vec<&str>,
) -> StructuredWorldUpdate {
    StructuredWorldUpdate {
        entities,
        relationships,
        state_changes,
        new_observations: new_observations
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        open_questions: vec![],
        possible_hypotheses: vec![],
    }
}

fn make_wm_entity(name: &str, entity_type: &str, importance: f64, confidence: f64) -> Entity {
    Entity::new(entity_type, name)
        .with_importance(importance as f32)
        .with_confidence(confidence as f32)
}

// ── Scenario 1: Long-term project (30 days) ──────────────────

/// A 30-day scenario where the user starts, works on, gets blocked by,
/// pauses, and finally ships a long-term project (AI-OS).
pub fn long_term_project() -> Scenario {
    let mut days = Vec::new();

    // Day 1: User starts AI-OS
    days.push(ScenarioDay::new(
        Some("I'm starting a new project called AI-OS. It's an operating system for AI."),
        Some(make_update(
            vec![make_entity("AI-OS", "project", 0.85, 0.8)],
            vec![],
            vec![],
            vec!["User started AI-OS project"],
        )),
    ));

    // Days 2-4: Progress
    for (i, obs) in [
        "Implemented the core scheduler for AI-OS",
        "Added memory management to AI-OS",
        "Working on the IPC layer for AI-OS",
    ]
    .iter()
    .enumerate()
    {
        days.push(ScenarioDay::new(
            Some(obs),
            Some(make_update(
                vec![make_entity(
                    "AI-OS",
                    "project",
                    0.85 + (i as f64 * 0.02),
                    0.85,
                )],
                vec![],
                vec![],
                vec!["Making progress on AI-OS"],
            )),
        ));
    }

    // Days 5-6: No interaction
    days.push(ScenarioDay::silent());
    days.push(ScenarioDay::silent());

    // Day 7: User returns, still working
    days.push(ScenarioDay::new(
        Some("Back to working on AI-OS. Implementing the networking stack."),
        Some(make_update(
            vec![make_entity("AI-OS", "project", 0.88, 0.85)],
            vec![],
            vec![],
            vec!["Resumed work on AI-OS networking"],
        )),
    ));

    // Days 8-14: Steady work
    for i in 0..7 {
        days.push(ScenarioDay::new(
            Some(&format!(
                "Day {} of AI-OS development. Making good progress on the filesystem module.",
                i + 8
            )),
            Some(make_update(
                vec![make_entity(
                    "AI-OS",
                    "project",
                    0.87 + (i as f64 * 0.01),
                    0.86,
                )],
                vec![],
                vec![],
                vec!["Steady progress on AI-OS filesystem"],
            )),
        ));
    }

    // Days 15-16: Blocked
    days.push(ScenarioDay::new(
        Some(
            "I'm blocked on AI-OS. The deployment pipeline is failing and I can't figure out why.",
        ),
        Some(make_update(
            vec![
                make_entity("AI-OS", "project", 0.9, 0.8),
                make_entity("Deployment", "task", 0.8, 0.5),
            ],
            vec![],
            vec![StateChange {
                entity_name: "AI-OS".into(),
                attribute: "status".into(),
                old_value: Some("active".into()),
                new_value: "blocked".into(),
            }],
            vec!["Blocked on AI-OS deployment"],
        )),
    ));

    // Days 17-20: User goes silent (frustrated or busy)
    for _ in 0..4 {
        days.push(ScenarioDay::silent());
    }

    // Day 21: User returns
    days.push(ScenarioDay::new(
        Some("I finally fixed the deployment issue! AI-OS is back on track."),
        Some(make_update(
            vec![make_entity("AI-OS", "project", 0.92, 0.9)],
            vec![],
            vec![StateChange {
                entity_name: "AI-OS".into(),
                attribute: "status".into(),
                old_value: Some("blocked".into()),
                new_value: "active".into(),
            }],
            vec!["Unblocked AI-OS deployment"],
        )),
    ));

    // Days 22-29: Final push
    for i in 0..8 {
        days.push(ScenarioDay::new(
            Some(&format!(
                "AI-OS development day {}. Wrapping up the final modules.",
                i + 22
            )),
            Some(make_update(
                vec![make_entity(
                    "AI-OS",
                    "project",
                    0.93 + (i as f64 * 0.005),
                    0.92,
                )],
                vec![],
                vec![],
                vec!["Final push on AI-OS"],
            )),
        ));
    }

    // Day 30: Ship it
    days.push(ScenarioDay::new(
        Some("AI-OS is done! Shipping the first release today."),
        Some(make_update(
            vec![make_entity("AI-OS", "project", 0.95, 0.95)],
            vec![],
            vec![StateChange {
                entity_name: "AI-OS".into(),
                attribute: "status".into(),
                old_value: Some("active".into()),
                new_value: "shipped".into(),
            }],
            vec!["AI-OS first release shipped"],
        )),
    ));

    Scenario {
        name: "Long-term project",
        description: "User starts, builds, gets blocked, pauses, and ships a long-term project (30 days)",
        initial_entities: vec![],
        days,
    }
}

// ── Scenario 2: Learning journey (30 days) ───────────────────

pub fn learning_journey() -> Scenario {
    let mut days = Vec::new();

    days.push(ScenarioDay::new(
        Some("I'm starting to learn Rust. Coming from Python, this feels very different."),
        Some(make_update(
            vec![
                make_entity("Rust", "skill", 0.8, 0.7),
                make_entity("Python", "skill", 0.6, 0.9),
            ],
            vec![WorldRelationship {
                source: "User".into(),
                target: "Rust".into(),
                relationship_type: "learning".into(),
                confidence: 0.7,
                weight: 0.8,
            }],
            vec![],
            vec!["Started learning Rust"],
        )),
    ));

    for (i, obs) in [
        "Rust's ownership system is confusing but I'm getting it.",
        "Borrow checker is fighting me, but I understand why it exists.",
        "Got my first Rust program to compile! It's a simple CLI tool.",
    ]
    .iter()
    .enumerate()
    {
        days.push(ScenarioDay::new(
            Some(obs),
            Some(make_update(
                vec![make_entity(
                    "Rust",
                    "skill",
                    0.75 + (i as f64 * 0.05),
                    0.7 + (i as f64 * 0.05),
                )],
                vec![],
                vec![],
                vec!["Progressing in Rust"],
            )),
        ));
    }

    // Days 5-7: No progress
    for _ in 0..3 {
        days.push(ScenarioDay::silent());
    }

    // Days 8-30: Mix of progress and silence
    let updates = [
        ("Working on a web server in Rust. Using Axum.", 0.82, 0.78),
        (
            "Rust's error handling with anyhow/thiserror is elegant.",
            0.84,
            0.80,
        ),
        ("Building a CLI tool with clap. Loving the DX.", 0.85, 0.82),
        (
            "Refactoring my Rust code to use more idiomatic patterns.",
            0.86,
            0.84,
        ),
        ("Reading 'Rust for Rustaceans'. Leveling up.", 0.88, 0.86),
        ("Contributed to an open-source Rust project!", 0.90, 0.90),
    ];

    for (obs, importance, confidence) in &updates {
        days.push(ScenarioDay::new(
            Some(obs),
            Some(make_update(
                vec![make_entity("Rust", "skill", *importance, *confidence)],
                vec![],
                vec![],
                vec!["Leveling up in Rust"],
            )),
        ));
        // Add silent days between updates
        for _ in 0..2 {
            days.push(ScenarioDay::silent());
        }
    }

    // Fill remaining days with silence
    while days.len() < 30 {
        days.push(ScenarioDay::silent());
    }

    Scenario {
        name: "Learning journey",
        description: "User learns a new programming language over 30 days with varying engagement",
        initial_entities: vec![],
        days: days.into_iter().take(30).collect(),
    }
}

// ── Scenario 3: Changing priorities (30 days) ────────────────

pub fn changing_priorities() -> Scenario {
    let mut days = Vec::new();

    // Project A: building a blog
    days.push(ScenarioDay::new(
        Some("I'm building a personal blog with Astro. Setting up the project."),
        Some(make_update(
            vec![make_entity("Personal Blog", "project", 0.8, 0.8)],
            vec![],
            vec![],
            vec!["Started personal blog project"],
        )),
    ));

    for i in 0..4 {
        days.push(ScenarioDay::new(
            Some(&format!(
                "Blog dev day {}: working on the theme and layout.",
                i + 1
            )),
            Some(make_update(
                vec![make_entity(
                    "Personal Blog",
                    "project",
                    0.78 + (i as f64 * 0.02),
                    0.8,
                )],
                vec![],
                vec![],
                vec!["Blog progress"],
            )),
        ));
    }

    // Priority shifts: Work project becomes urgent
    days.push(ScenarioDay::new(
        Some("Urgent! The work project has a critical deadline next week."),
        Some(make_update(
            vec![
                make_entity("Work Project", "project", 0.95, 0.9),
                make_entity("Personal Blog", "project", 0.6, 0.7),
            ],
            vec![],
            vec![StateChange {
                entity_name: "Work Project".into(),
                attribute: "priority".into(),
                old_value: Some("normal".into()),
                new_value: "urgent".into(),
            }],
            vec!["Work project became urgent"],
        )),
    ));

    for i in 0..5 {
        days.push(ScenarioDay::new(
            Some(&format!(
                "Crunching on work project. Long hours day {}.",
                i + 1
            )),
            Some(make_update(
                vec![make_entity(
                    "Work Project",
                    "project",
                    0.93 - (i as f64 * 0.02),
                    0.85,
                )],
                vec![],
                vec![],
                vec!["Working on urgent work project"],
            )),
        ));
    }

    // Work project delivered, back to blog
    days.push(ScenarioDay::new(
        Some("Work project delivered! Now I can get back to my blog."),
        Some(make_update(
            vec![
                make_entity("Work Project", "project", 0.7, 0.9),
                make_entity("Personal Blog", "project", 0.75, 0.75),
            ],
            vec![],
            vec![StateChange {
                entity_name: "Work Project".into(),
                attribute: "status".into(),
                old_value: Some("active".into()),
                new_value: "completed".into(),
            }],
            vec!["Work project completed, returning to blog"],
        )),
    ));

    for i in 0..5 {
        days.push(ScenarioDay::new(
            Some(&format!(
                "Blog dev day {}: adding RSS and SEO features.",
                i + 6
            )),
            Some(make_update(
                vec![make_entity(
                    "Personal Blog",
                    "project",
                    0.78 + (i as f64 * 0.02),
                    0.82,
                )],
                vec![],
                vec![],
                vec!["Blog progress resumed"],
            )),
        ));
    }

    // Fill with silence
    while days.len() < 30 {
        days.push(ScenarioDay::silent());
    }

    Scenario {
        name: "Changing priorities",
        description: "User switches between projects as priorities change (30 days)",
        initial_entities: vec![],
        days: days.into_iter().take(30).collect(),
    }
}

// ── Scenario 4: Stress & inactivity (14 days) ────────────────

pub fn stress_and_inactivity() -> Scenario {
    let mut days = Vec::new();

    days.push(ScenarioDay::new(
        Some("I'm feeling overwhelmed. Too many things to do and not enough time."),
        Some(make_update(
            vec![make_entity("User", "user", 0.7, 0.8)],
            vec![],
            vec![StateChange {
                entity_name: "User".into(),
                attribute: "mood".into(),
                old_value: Some("neutral".into()),
                new_value: "stressed".into(),
            }],
            vec!["User feeling overwhelmed"],
        )),
    ));

    days.push(ScenarioDay::new(
        Some("I'm exhausted. Taking a break for a few days."),
        Some(make_update(
            vec![make_entity("User", "user", 0.6, 0.8)],
            vec![],
            vec![StateChange {
                entity_name: "User".into(),
                attribute: "mood".into(),
                old_value: Some("stressed".into()),
                new_value: "tired".into(),
            }],
            vec!["User needs a break"],
        )),
    ));

    // 4 days of complete silence
    for _ in 0..4 {
        days.push(ScenarioDay::silent());
    }

    days.push(ScenarioDay::new(
        Some("I'm feeling better. Ready to get back to work."),
        Some(make_update(
            vec![make_entity("User", "user", 0.8, 0.9)],
            vec![],
            vec![StateChange {
                entity_name: "User".into(),
                attribute: "mood".into(),
                old_value: Some("tired".into()),
                new_value: "energized".into(),
            }],
            vec!["User is back and energized"],
        )),
    ));

    for i in 0..5 {
        days.push(ScenarioDay::new(
            Some(&format!(
                "Back to work day {}. Catching up on everything.",
                i + 1
            )),
            Some(make_update(
                vec![make_entity("User", "user", 0.82 + (i as f64 * 0.02), 0.88)],
                vec![],
                vec![],
                vec!["Catching up after break"],
            )),
        ));
    }

    while days.len() < 14 {
        days.push(ScenarioDay::silent());
    }

    Scenario {
        name: "Stress & inactivity",
        description: "User experiences stress, takes a break, and returns (14 days)",
        initial_entities: vec![],
        days: days.into_iter().take(14).collect(),
    }
}

// ── Scenario 5: Success & failure (14 days) ──────────────────

pub fn success_and_failure() -> Scenario {
    let mut days = Vec::new();

    // Initial entity to give context
    let initial = vec![make_wm_entity("Side Project", "project", 0.8, 0.85)];

    days.push(ScenarioDay::new(
        Some("I launched my side project! Initial user feedback is amazing."),
        Some(make_update(
            vec![make_entity("Side Project", "project", 0.9, 0.9)],
            vec![],
            vec![StateChange {
                entity_name: "Side Project".into(),
                attribute: "status".into(),
                old_value: Some("development".into()),
                new_value: "launched".into(),
            }],
            vec!["Side project launched successfully"],
        )),
    ));

    for i in 0..3 {
        days.push(ScenarioDay::new(
            Some(&format!(
                "Day {} post-launch: Users love it! Getting great feedback.",
                i + 1
            )),
            Some(make_update(
                vec![make_entity(
                    "Side Project",
                    "project",
                    0.91 + (i as f64 * 0.01),
                    0.92,
                )],
                vec![],
                vec![],
                vec!["Positive user feedback"],
            )),
        ));
    }

    // Crisis
    days.push(ScenarioDay::new(
        Some("Critical bug found! A security vulnerability was discovered. Users are at risk."),
        Some(make_update(
            vec![
                make_entity("Side Project", "project", 0.95, 0.6),
                make_entity("Security Vulnerability", "issue", 0.95, 0.9),
            ],
            vec![],
            vec![StateChange {
                entity_name: "Side Project".into(),
                attribute: "status".into(),
                old_value: Some("launched".into()),
                new_value: "critical".into(),
            }],
            vec!["Critical security vulnerability found"],
        )),
    ));

    for i in 0..3 {
        days.push(ScenarioDay::new(
            Some(&format!(
                "Working on the security fix. Day {} of the crisis.",
                i + 1
            )),
            Some(make_update(
                vec![make_entity(
                    "Side Project",
                    "project",
                    0.88 - (i as f64 * 0.05),
                    0.6,
                )],
                vec![],
                vec![],
                vec!["Working on security fix"],
            )),
        ));
    }

    days.push(ScenarioDay::new(
        Some("Security fix deployed. Users are coming back. Lesson learned."),
        Some(make_update(
            vec![make_entity("Side Project", "project", 0.85, 0.85)],
            vec![],
            vec![StateChange {
                entity_name: "Side Project".into(),
                attribute: "status".into(),
                old_value: Some("critical".into()),
                new_value: "stable".into(),
            }],
            vec!["Security fix deployed, recovery underway"],
        )),
    ));

    while days.len() < 14 {
        days.push(ScenarioDay::silent());
    }

    Scenario {
        name: "Success & failure",
        description: "User experiences launch success, a critical bug, and recovery (14 days)",
        initial_entities: initial,
        days: days.into_iter().take(14).collect(),
    }
}

// ── Scenario 6: Interrupted work (7 days) ────────────────────

pub fn interrupted_work() -> Scenario {
    let mut days = Vec::new();

    days.push(ScenarioDay::new(
        Some("Deep focus on refactoring the database layer today."),
        Some(make_update(
            vec![make_entity("Database Refactor", "project", 0.85, 0.8)],
            vec![],
            vec![],
            vec!["Deep focus on database refactoring"],
        )),
    ));

    days.push(ScenarioDay::new(
        Some("Interrupted by a production incident. Need to fix it urgently."),
        Some(make_update(
            vec![
                make_entity("Production Incident", "issue", 0.95, 0.95),
                make_entity("Database Refactor", "project", 0.7, 0.7),
            ],
            vec![],
            vec![StateChange {
                entity_name: "Database Refactor".into(),
                attribute: "status".into(),
                old_value: Some("active".into()),
                new_value: "paused".into(),
            }],
            vec!["Interrupted by production incident"],
        )),
    ));

    for i in 0..3 {
        days.push(ScenarioDay::new(
            Some(&format!(
                "Incident response day {}: Root cause identified, deploying fix.",
                i + 1
            )),
            Some(make_update(
                vec![make_entity(
                    "Production Incident",
                    "issue",
                    0.8 - (i as f64 * 0.15),
                    0.85,
                )],
                vec![],
                vec![],
                vec!["Working on incident resolution"],
            )),
        ));
    }

    days.push(ScenarioDay::new(
        Some("Incident resolved. Back to the database refactor now."),
        Some(make_update(
            vec![
                make_entity("Database Refactor", "project", 0.82, 0.82),
                make_entity("Production Incident", "issue", 0.3, 0.95),
            ],
            vec![],
            vec![StateChange {
                entity_name: "Database Refactor".into(),
                attribute: "status".into(),
                old_value: Some("paused".into()),
                new_value: "active".into(),
            }],
            vec!["Resumed database refactoring after incident"],
        )),
    ));

    while days.len() < 7 {
        days.push(ScenarioDay::silent());
    }

    Scenario {
        name: "Interrupted work",
        description: "User is deep in focus, gets interrupted by an incident, resolves it, and returns (7 days)",
        initial_entities: vec![],
        days: days.into_iter().take(7).collect(),
    }
}

// ── Scenario 7: Relationship follow-up (14 days) ─────────────

pub fn relationship_follow_up() -> Scenario {
    let mut days = Vec::new();

    days.push(ScenarioDay::new(
        Some("Alice helped me with the UI design. She was incredibly helpful."),
        Some(make_update(
            vec![make_entity("Alice", "person", 0.85, 0.9)],
            vec![WorldRelationship {
                source: "User".into(),
                target: "Alice".into(),
                relationship_type: "collaborator".into(),
                confidence: 0.85,
                weight: 0.8,
            }],
            vec![],
            vec!["Alice helped with UI design"],
        )),
    ));

    days.push(ScenarioDay::new(
        Some("I should thank Alice properly. Maybe invite her for coffee."),
        Some(make_update(
            vec![make_entity("Alice", "person", 0.88, 0.9)],
            vec![WorldRelationship {
                source: "User".into(),
                target: "Alice".into(),
                relationship_type: "collaborator".into(),
                confidence: 0.9,
                weight: 0.85,
            }],
            vec![],
            vec!["Planning to thank Alice"],
        )),
    ));

    // 3 days of silence
    for _ in 0..3 {
        days.push(ScenarioDay::silent());
    }

    days.push(ScenarioDay::new(
        Some("Had coffee with Alice. She's interested in collaborating more."),
        Some(make_update(
            vec![make_entity("Alice", "person", 0.9, 0.92)],
            vec![WorldRelationship {
                source: "User".into(),
                target: "Alice".into(),
                relationship_type: "close_collaborator".into(),
                confidence: 0.92,
                weight: 0.9,
            }],
            vec![],
            vec!["Coffee with Alice was productive"],
        )),
    ));

    // Fill with silence
    while days.len() < 14 {
        days.push(ScenarioDay::silent());
    }

    Scenario {
        name: "Relationship follow-up",
        description: "User mentions a person, plans to follow up, and the relationship deepens (14 days)",
        initial_entities: vec![],
        days: days.into_iter().take(14).collect(),
    }
}
