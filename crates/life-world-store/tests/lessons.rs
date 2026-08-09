//! Lessons: durable upsert, replacement semantics, and listing.

mod common;

use common::{Sandbox, lesson};

/// upsert_lesson writes and get_lesson round-trips the lesson.
#[tokio::test]
async fn upsert_and_get_lesson_roundtrip() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let l = lesson();
    let stored = sb.store.upsert_lesson(l.clone()).await.unwrap();
    assert_eq!(stored, l);

    let got = sb.store.get_lesson(&l.lesson_id).await.unwrap().unwrap();
    assert_eq!(got, l);
}

/// get_lesson returns None for an unknown id.
#[tokio::test]
async fn get_missing_lesson_returns_none() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let ghost = uuid::Uuid::new_v4().to_string();
    assert!(sb.store.get_lesson(&ghost).await.unwrap().is_none());
}

/// Re-writing the same lesson id replaces the previous content.
#[tokio::test]
async fn upsert_lesson_replaces() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let l = lesson();
    sb.store.upsert_lesson(l.clone()).await.unwrap();

    let mut revised = l.clone();
    revised.content = "A revised lesson".into();
    revised.category = "safety".into();
    let stored = sb.store.upsert_lesson(revised.clone()).await.unwrap();
    assert_eq!(stored, revised);

    let got = sb.store.get_lesson(&l.lesson_id).await.unwrap().unwrap();
    assert_eq!(got.content, "A revised lesson");
    assert_eq!(got.category, "safety");
    // Single row for the id.
    let conn = sb.raw();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM lessons WHERE lesson_id = ?1",
            [l.lesson_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

/// all_lessons lists every lesson, newest first.
#[tokio::test]
async fn all_lessons_lists_and_orders() {
    use memory_core::Timestamp;
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let mut l1 = lesson();
    l1.created_at = Timestamp::from_secs(1_700_000_000);
    sb.store.upsert_lesson(l1.clone()).await.unwrap();

    let mut l2 = lesson();
    l2.created_at = Timestamp::from_secs(1_700_000_100);
    sb.store.upsert_lesson(l2.clone()).await.unwrap();

    let all = sb.store.all_lessons().await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].lesson_id, l2.lesson_id, "newest first");
    assert_eq!(all[1].lesson_id, l1.lesson_id);
}

/// Lessons survive a close/reopen cycle.
#[tokio::test]
async fn lessons_survive_restart() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let stored = sb.store.upsert_lesson(lesson()).await.unwrap();

    let store = sb.reopen().await;
    let got = store.get_lesson(&stored.lesson_id).await.unwrap().unwrap();
    assert_eq!(got, stored);
}
