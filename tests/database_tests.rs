use orgasmus_tracker::database::{Database, DbError};
use tempfile::NamedTempFile;

async fn create_test_db() -> (Database, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap().to_string();
    let db = Database::connect(&db_path).await.unwrap();
    (db, temp_file)
}

#[tokio::test]
async fn test_init_db() {
    let (db, _tmp) = create_test_db().await;

    // Verify tables exist by inserting directly
    let res = db
        .add_entry(123, "simple", "2026-10-19 18:30", None, None, None, None, None)
        .await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_add_simple_entry() {
    let (db, _tmp) = create_test_db().await;

    let id = db
        .add_entry(100, "simple", "2026-10-19 18:30", None, None, None, None, None)
        .await
        .unwrap();
    assert!(id > 0);

    let entry = db.get_entry_by_id(id, 100).await.unwrap().unwrap();
    assert_eq!(entry.id, id);
    assert_eq!(entry.user_id, 100);
    assert_eq!(entry.r#type, "simple");
    assert_eq!(entry.timestamp, "2026-10-19 18:30");
    assert!(entry.company.is_none());
    assert!(entry.partner_name.is_none());
    assert!(entry.quality.is_none());
    assert!(entry.description.is_none());
}

#[tokio::test]
async fn test_add_detailed_entry() {
    let (db, _tmp) = create_test_db().await;

    let id = db
        .add_entry(
            200,
            "detailed",
            "2026-10-19 20:00",
            Some("Mit anderer Person"),
            Some("Julia"),
            Some(9),
            Some("Traumhaft schön!"),
            Some("2026-10-19T20:01:00Z"),
        )
        .await
        .unwrap();

    let entry = db.get_entry_by_id(id, 200).await.unwrap().unwrap();
    assert_eq!(entry.id, id);
    assert_eq!(entry.user_id, 200);
    assert_eq!(entry.r#type, "detailed");
    assert_eq!(entry.company.as_deref(), Some("Mit anderer Person"));
    assert_eq!(entry.partner_name.as_deref(), Some("Julia"));
    assert_eq!(entry.quality, Some(9));
    assert_eq!(entry.description.as_deref(), Some("Traumhaft schön!"));
    assert_eq!(entry.created_at, "2026-10-19T20:01:00Z");
}

#[tokio::test]
async fn test_invalid_type_raises_error() {
    let (db, _tmp) = create_test_db().await;

    let res = db
        .add_entry(100, "invalid_type", "2026-10-19 12:00", None, None, None, None, None)
        .await;
    assert!(matches!(res, Err(DbError::InvalidType(_))));
}

#[tokio::test]
async fn test_invalid_quality_raises_error() {
    let (db, _tmp) = create_test_db().await;

    let res_low = db
        .add_entry(100, "detailed", "2026-10-19 12:00", None, None, Some(0), None, None)
        .await;
    assert!(matches!(res_low, Err(DbError::InvalidQuality(0))));

    let res_high = db
        .add_entry(100, "detailed", "2026-10-19 12:00", None, None, Some(11), None, None)
        .await;
    assert!(matches!(res_high, Err(DbError::InvalidQuality(11))));
}

#[tokio::test]
async fn test_get_last_entries_ordering_and_limit() {
    let (db, _tmp) = create_test_db().await;

    for i in 1..=5 {
        db.add_entry(
            300,
            "simple",
            &format!("2026-10-19 1{}:00", i),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    }

    let entries = db.get_last_entries(300, 3).await.unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].timestamp, "2026-10-19 15:00");
    assert_eq!(entries[1].timestamp, "2026-10-19 14:00");
    assert_eq!(entries[2].timestamp, "2026-10-19 13:00");
}

#[tokio::test]
async fn test_isolation_between_users() {
    let (db, _tmp) = create_test_db().await;

    let id_a = db
        .add_entry(111, "simple", "2026-10-19 10:00", None, None, None, None, None)
        .await
        .unwrap();

    let id_b = db
        .add_entry(222, "simple", "2026-10-19 11:00", None, None, None, None, None)
        .await
        .unwrap();

    // User A cannot fetch User B's entry
    assert!(db.get_entry_by_id(id_b, 111).await.unwrap().is_none());
    assert!(db.get_entry_by_id(id_a, 222).await.unwrap().is_none());

    let entries_a = db.get_last_entries(111, 10).await.unwrap();
    assert_eq!(entries_a.len(), 1);
    assert_eq!(entries_a[0].id, id_a);
}

#[tokio::test]
async fn test_delete_entry() {
    let (db, _tmp) = create_test_db().await;

    let id = db
        .add_entry(400, "simple", "2026-10-19 12:00", None, None, None, None, None)
        .await
        .unwrap();

    // Wrong user cannot delete
    let deleted_wrong = db.delete_entry(id, 999).await.unwrap();
    assert!(!deleted_wrong);
    assert!(db.get_entry_by_id(id, 400).await.unwrap().is_some());

    // Right user deletes
    let deleted = db.delete_entry(id, 400).await.unwrap();
    assert!(deleted);
    assert!(db.get_entry_by_id(id, 400).await.unwrap().is_none());
}

#[tokio::test]
async fn test_delete_entries_empty() {
    let (db, _tmp) = create_test_db().await;
    let count = db.delete_entries(&[], 500).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_delete_entries_batch() {
    let (db, _tmp) = create_test_db().await;

    let id1 = db
        .add_entry(600, "simple", "2026-10-19 10:00", None, None, None, None, None)
        .await
        .unwrap();
    let id2 = db
        .add_entry(600, "simple", "2026-10-19 11:00", None, None, None, None, None)
        .await
        .unwrap();
    let id3 = db
        .add_entry(600, "simple", "2026-10-19 12:00", None, None, None, None, None)
        .await
        .unwrap();

    let deleted = db.delete_entries(&[id1, id2], 600).await.unwrap();
    assert_eq!(deleted, 2);

    let remaining = db.get_last_entries(600, 10).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, id3);
}

#[tokio::test]
async fn test_delete_entries_isolation() {
    let (db, _tmp) = create_test_db().await;

    let id_a = db
        .add_entry(700, "simple", "2026-10-19 10:00", None, None, None, None, None)
        .await
        .unwrap();
    let id_b = db
        .add_entry(800, "simple", "2026-10-19 11:00", None, None, None, None, None)
        .await
        .unwrap();

    // User A attempts to batch delete both id_a and id_b
    let deleted = db.delete_entries(&[id_a, id_b], 700).await.unwrap();
    assert_eq!(deleted, 1);

    assert!(db.get_entry_by_id(id_a, 700).await.unwrap().is_none());
    assert!(db.get_entry_by_id(id_b, 800).await.unwrap().is_some());
}

#[tokio::test]
async fn test_get_all_entries() {
    let (db, _tmp) = create_test_db().await;

    // Initially empty
    let empty = db.get_all_entries(100).await.unwrap();
    assert!(empty.is_empty());

    // Insert entries out of chronological order
    db.add_entry(100, "simple", "2026-10-19 12:00", None, None, None, None, None)
        .await
        .unwrap();
    db.add_entry(100, "simple", "2026-10-18 09:00", None, None, None, None, None)
        .await
        .unwrap();
    db.add_entry(100, "simple", "2026-10-20 15:30", None, None, None, None, None)
        .await
        .unwrap();

    // Another user's entry
    db.add_entry(200, "simple", "2026-10-19 14:00", None, None, None, None, None)
        .await
        .unwrap();

    let entries = db.get_all_entries(100).await.unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].timestamp, "2026-10-18 09:00");
    assert_eq!(entries[1].timestamp, "2026-10-19 12:00");
    assert_eq!(entries[2].timestamp, "2026-10-20 15:30");

    let entries_other = db.get_all_entries(200).await.unwrap();
    assert_eq!(entries_other.len(), 1);
    assert_eq!(entries_other[0].timestamp, "2026-10-19 14:00");
}

#[tokio::test]
async fn test_get_statistics_empty() {
    let (db, _tmp) = create_test_db().await;
    let tz = chrono_tz::Europe::Berlin;
    let now = chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 10, 20, 12, 0, 0).unwrap();

    let stats = db.get_statistics_at(999, now).await.unwrap();
    assert_eq!(stats.total_count, 0);
    assert_eq!(stats.count_last_7_days, 0);
    assert_eq!(stats.count_last_30_days, 0);
    assert_eq!(stats.count_current_year, 0);
    assert_eq!(stats.count_last_365_days, 0);
    assert_eq!(stats.count_previous_year, 0);
    assert_eq!(stats.count_alone, 0);
    assert_eq!(stats.count_partner, 0);
    assert_eq!(stats.avg_quality, None);
}

#[tokio::test]
async fn test_get_statistics_aggregation() {
    let (db, _tmp) = create_test_db().await;
    let tz = chrono_tz::Europe::Berlin;
    let now = chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 10, 20, 12, 0, 0).unwrap();
    let user_id = 12345;

    // Entry 1: Today (within 7 days, 30 days, current year, last 365 days) - Alone, quality 8
    db.add_entry(
        user_id,
        "detailed",
        "2026-10-20 10:00",
        Some("Alleine"),
        None,
        Some(8),
        None,
        None,
    )
    .await
    .unwrap();

    // Entry 2: 5 days ago (within 7 days, 30 days, current year, last 365 days) - Partner, quality 9
    db.add_entry(
        user_id,
        "detailed",
        "2026-10-15 14:00",
        Some("Mit anderer Person"),
        Some("Alex"),
        Some(9),
        None,
        None,
    )
    .await
    .unwrap();

    // Entry 3: 15 days ago (within 30 days, current year, last 365 days; not 7 days) - Simple, no quality
    db.add_entry(
        user_id,
        "simple",
        "2026-10-05 08:30",
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // Entry 4: 60 days ago (current year, last 365 days; not 30 days) - Partner, quality 7
    db.add_entry(
        user_id,
        "detailed",
        "2026-08-20 22:00",
        Some("Mit anderer Person"),
        Some("Sam"),
        Some(7),
        None,
        None,
    )
    .await
    .unwrap();

    // Entry 5: Previous calendar year (2025-11-15, within last 365 days and previous_year) - Alone, quality 10
    db.add_entry(
        user_id,
        "detailed",
        "2025-11-15 19:00",
        Some("Alleine"),
        None,
        Some(10),
        None,
        None,
    )
    .await
    .unwrap();

    // Entry 6: Previous year but > 365 days ago (2025-05-01, previous year; NOT last 365 days) - Simple
    db.add_entry(
        user_id,
        "simple",
        "2025-05-01 12:00",
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // Entry 7: 2024 (two years ago - only in total_count) - Partner, quality 6
    db.add_entry(
        user_id,
        "detailed",
        "2024-06-01 12:00",
        Some("Mit anderer Person"),
        Some("Sam"),
        Some(6),
        None,
        None,
    )
    .await
    .unwrap();

    // Different user entry (must be completely ignored)
    db.add_entry(
        99999,
        "detailed",
        "2026-10-20 11:00",
        Some("Alleine"),
        None,
        Some(1),
        None,
        None,
    )
    .await
    .unwrap();

    let stats = db.get_statistics_at(user_id, now).await.unwrap();

    assert_eq!(stats.total_count, 7);
    assert_eq!(stats.count_last_7_days, 2); // Entries 1, 2
    assert_eq!(stats.count_last_30_days, 3); // Entries 1, 2, 3
    assert_eq!(stats.count_current_year, 4); // Entries 1, 2, 3, 4
    assert_eq!(stats.count_last_365_days, 5); // Entries 1, 2, 3, 4, 5
    assert_eq!(stats.count_previous_year, 2); // Entries 5, 6
    assert_eq!(stats.count_alone, 2); // Entries 1, 5
    assert_eq!(stats.count_partner, 3); // Entries 2, 4, 7

    // Quality ratings for user_id: 8, 9, 7, 10, 6 -> sum = 40, count = 5 -> average = 8.0
    assert_eq!(stats.avg_quality, Some(8.0));
}

