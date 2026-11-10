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
        .add_entry(123, "simple", "2026-10-19 18:30", None, None, None, None, None, None)
        .await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_add_simple_entry() {
    let (db, _tmp) = create_test_db().await;

    let id = db
        .add_entry(100, "simple", "2026-10-19 18:30", None, None, None, None, None, None)
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
    assert!(entry.location.is_none());
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
            Some("Nils"),
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
    assert_eq!(entry.location.as_deref(), Some("Nils"));
    assert_eq!(entry.quality, Some(9));
    assert_eq!(entry.description.as_deref(), Some("Traumhaft schön!"));
    assert_eq!(entry.created_at, "2026-10-19T20:01:00Z");
}

#[tokio::test]
async fn test_invalid_type_raises_error() {
    let (db, _tmp) = create_test_db().await;

    let res = db
        .add_entry(100, "invalid_type", "2026-10-19 12:00", None, None, None, None, None, None)
        .await;
    assert!(matches!(res, Err(DbError::InvalidType(_))));
}

#[tokio::test]
async fn test_invalid_quality_raises_error() {
    let (db, _tmp) = create_test_db().await;

    let res_low = db
        .add_entry(100, "detailed", "2026-10-19 12:00", None, None, None, Some(0), None, None)
        .await;
    assert!(matches!(res_low, Err(DbError::InvalidQuality(0))));

    let res_high = db
        .add_entry(100, "detailed", "2026-10-19 12:00", None, None, None, Some(11), None, None)
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
        .add_entry(111, "simple", "2026-10-19 10:00", None, None, None, None, None, None)
        .await
        .unwrap();

    let id_b = db
        .add_entry(222, "simple", "2026-10-19 11:00", None, None, None, None, None, None)
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
        .add_entry(400, "simple", "2026-10-19 12:00", None, None, None, None, None, None)
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
        .add_entry(600, "simple", "2026-10-19 10:00", None, None, None, None, None, None)
        .await
        .unwrap();
    let id2 = db
        .add_entry(600, "simple", "2026-10-19 11:00", None, None, None, None, None, None)
        .await
        .unwrap();
    let id3 = db
        .add_entry(600, "simple", "2026-10-19 12:00", None, None, None, None, None, None)
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
        .add_entry(700, "simple", "2026-10-19 10:00", None, None, None, None, None, None)
        .await
        .unwrap();
    let id_b = db
        .add_entry(800, "simple", "2026-10-19 11:00", None, None, None, None, None, None)
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
    db.add_entry(100, "simple", "2026-10-19 12:00", None, None, None, None, None, None)
        .await
        .unwrap();
    db.add_entry(100, "simple", "2026-10-18 09:00", None, None, None, None, None, None)
        .await
        .unwrap();
    db.add_entry(100, "simple", "2026-10-20 15:30", None, None, None, None, None, None)
        .await
        .unwrap();

    // Another user's entry
    db.add_entry(200, "simple", "2026-10-19 14:00", None, None, None, None, None, None)
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
        Some("Nils"),
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
        Some("Mika"),
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
        Some("Hotel"),
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
        None,
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

#[tokio::test]
async fn test_migration_adds_location_column() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap().to_string();

    // Simulate an older database schema without the location column
    {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect(&format!("sqlite://{}", db_path))
            .await
            .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                company TEXT NULL,
                partner_name TEXT NULL,
                quality INTEGER NULL,
                description TEXT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert legacy row without location
        sqlx::query(
            r#"
            INSERT INTO entries (user_id, type, timestamp, company, partner_name, quality, description, created_at)
            VALUES (1, 'simple', '2026-10-19 12:00', NULL, NULL, NULL, NULL, '2026-10-19T12:00:00Z');
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        pool.close().await;
    }

    // Now connect with Database::connect which calls init_db()
    let db = Database::connect(&db_path).await.unwrap();

    // Check that legacy entry can be read and location is None
    let entries = db.get_last_entries(1, 10).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].location, None);

    // Insert new entry with location
    let new_id = db
        .add_entry(
            1,
            "detailed",
            "2026-10-20 10:00",
            Some("Alleine"),
            None,
            Some("Nils"),
            Some(9),
            None,
            None,
        )
        .await
        .unwrap();

    let fetched = db.get_entry_by_id(new_id, 1).await.unwrap().unwrap();
    assert_eq!(fetched.location.as_deref(), Some("Nils"));
}

#[tokio::test]
async fn test_statistics_streaks() {
    let (db, _tmp) = create_test_db().await;
    let tz = chrono_tz::Europe::Berlin;
    // Reference time: 2026-10-20 15:00:00 (Tuesday)
    let now = chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 10, 20, 15, 0, 0).unwrap();

    // User 1: Active streak ending today (2026-10-18, 2026-10-19, 2026-10-20)
    // Multiple entries on 2026-10-19 and 2026-10-20 to ensure deduplication works
    let user_active_today = 101;
    db.add_entry(user_active_today, "simple", "2026-10-18 10:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_active_today, "simple", "2026-10-19 09:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_active_today, "simple", "2026-10-19 21:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_active_today, "simple", "2026-10-20 08:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_active_today, "simple", "2026-10-20 14:00", None, None, None, None, None, None).await.unwrap();

    let stats1 = db.get_statistics_at(user_active_today, now).await.unwrap();
    assert_eq!(stats1.current_streak_days, 3);
    assert_eq!(stats1.longest_streak_days, 3);
    assert_eq!(stats1.days_since_last, Some(0)); // Today

    // User 2: Active streak ending yesterday (2026-10-18, 2026-10-19; none today)
    let user_active_yesterday = 102;
    db.add_entry(user_active_yesterday, "simple", "2026-10-18 12:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_active_yesterday, "simple", "2026-10-19 18:00", None, None, None, None, None, None).await.unwrap();

    let stats2 = db.get_statistics_at(user_active_yesterday, now).await.unwrap();
    assert_eq!(stats2.current_streak_days, 2);
    assert_eq!(stats2.longest_streak_days, 2);
    assert_eq!(stats2.days_since_last, Some(1)); // Yesterday

    // User 3: Broken streak (last entry 2 days ago: 2026-10-18; longer streak in past: 2026-10-10, 11, 12, 13 = 4 days)
    let user_broken = 103;
    db.add_entry(user_broken, "simple", "2026-10-10 10:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_broken, "simple", "2026-10-11 10:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_broken, "simple", "2026-10-12 10:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_broken, "simple", "2026-10-13 10:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_broken, "simple", "2026-10-17 10:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_broken, "simple", "2026-10-18 10:00", None, None, None, None, None, None).await.unwrap();

    let stats3 = db.get_statistics_at(user_broken, now).await.unwrap();
    assert_eq!(stats3.current_streak_days, 0); // Broken (last was 2 days ago)
    assert_eq!(stats3.longest_streak_days, 4); // Oct 10-13
    assert_eq!(stats3.days_since_last, Some(2));
}

#[tokio::test]
async fn test_statistics_intervals_and_gaps() {
    let (db, _tmp) = create_test_db().await;
    let tz = chrono_tz::Europe::Berlin;
    let now = chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 10, 20, 12, 0, 0).unwrap();

    // 0 entries
    let stats0 = db.get_statistics_at(300, now).await.unwrap();
    assert_eq!(stats0.longest_gap_days, None);
    assert_eq!(stats0.avg_days_between, None);

    // 1 entry
    db.add_entry(301, "simple", "2026-10-20 10:00", None, None, None, None, None, None).await.unwrap();
    let stats1 = db.get_statistics_at(301, now).await.unwrap();
    assert_eq!(stats1.longest_gap_days, None);
    assert_eq!(stats1.avg_days_between, None);

    // 2 entries on the same day (gap is 0, avg_days_between is 8 hours / 24 hours = 0.3 days)
    db.add_entry(302, "simple", "2026-10-20 10:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(302, "simple", "2026-10-20 18:00", None, None, None, None, None, None).await.unwrap();
    let stats2 = db.get_statistics_at(302, now).await.unwrap();
    assert_eq!(stats2.longest_gap_days, Some(0));
    assert_eq!(stats2.avg_days_between, Some(0.3));

    // Multiple entries across days:
    // 2026-10-01 12:00
    // 2026-10-05 12:00 (gap: 4 days)
    // 2026-10-06 12:00 (gap: 1 day)
    // Total duration: 5 days. (n - 1) = 2. avg = 2.5 days. Max gap = 4 days.
    db.add_entry(303, "simple", "2026-10-01 12:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(303, "simple", "2026-10-05 12:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(303, "simple", "2026-10-06 12:00", None, None, None, None, None, None).await.unwrap();
    let stats3 = db.get_statistics_at(303, now).await.unwrap();
    assert_eq!(stats3.longest_gap_days, Some(4));
    assert_eq!(stats3.avg_days_between, Some(2.5));
}

#[tokio::test]
async fn test_statistics_top_locations_and_partners() {
    let (db, _tmp) = create_test_db().await;
    let tz = chrono_tz::Europe::Berlin;
    let now = chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 10, 20, 12, 0, 0).unwrap();
    let user_id = 400;

    // Top Locations:
    // "Home": 3 entries, qualities: 8, 9, 10 -> avg = 9.0
    // "Hotel": 2 entries, qualities: 10, 10 -> avg = 10.0
    // "Beach": 2 entries, qualities: 7, 7 -> avg = 7.0
    // "Cabin": 1 entry, quality: 10 -> avg = 10.0 (should not be in top 3)
    // Empty & whitespace locations: should be ignored
    db.add_entry(user_id, "detailed", "2026-10-01 10:00", None, None, Some("Home"), Some(8), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-02 10:00", None, None, Some("Home"), Some(9), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-03 10:00", None, None, Some("  Home  "), Some(10), None, None).await.unwrap();

    db.add_entry(user_id, "detailed", "2026-10-04 10:00", None, None, Some("Hotel"), Some(10), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-05 10:00", None, None, Some("Hotel"), Some(10), None, None).await.unwrap();

    db.add_entry(user_id, "detailed", "2026-10-06 10:00", None, None, Some("Beach"), Some(7), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-07 10:00", None, None, Some("Beach"), Some(7), None, None).await.unwrap();

    db.add_entry(user_id, "detailed", "2026-10-08 10:00", None, None, Some("Cabin"), Some(10), None, None).await.unwrap();

    // Ignored locations:
    db.add_entry(user_id, "detailed", "2026-10-09 10:00", None, None, None, Some(10), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-10 10:00", None, None, Some(""), Some(10), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-11 10:00", None, None, Some("   "), Some(10), None, None).await.unwrap();

    // Top Partners:
    // "Julia": 3 entries with company = 'Mit anderer Person', qualities: 9, 9, None -> avg = 9.0
    // "Sarah": 2 entries with company = 'Mit anderer Person', qualities: 10, 10 -> avg = 10.0
    // "Alex": 2 entries with company = 'Mit anderer Person', qualities: 8, 8 -> avg = 8.0
    // "Mika": 1 entry with company = 'Mit anderer Person' -> not in top 3
    // "Emma": company = 'Alleine' -> MUST NOT BE INCLUDED!
    // Empty partner name -> ignored
    db.add_entry(user_id, "detailed", "2026-10-01 10:00", Some("Mit anderer Person"), Some("Julia"), None, Some(9), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-02 10:00", Some("Mit anderer Person"), Some(" Julia "), None, Some(9), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-03 10:00", Some("Mit anderer Person"), Some("Julia"), None, None, None, None).await.unwrap();

    db.add_entry(user_id, "detailed", "2026-10-04 10:00", Some("Mit anderer Person"), Some("Sarah"), None, Some(10), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-05 10:00", Some("Mit anderer Person"), Some("Sarah"), None, Some(10), None, None).await.unwrap();

    db.add_entry(user_id, "detailed", "2026-10-06 10:00", Some("Mit anderer Person"), Some("Alex"), None, Some(8), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-07 10:00", Some("Mit anderer Person"), Some("Alex"), None, Some(8), None, None).await.unwrap();

    db.add_entry(user_id, "detailed", "2026-10-08 10:00", Some("Mit anderer Person"), Some("Mika"), None, Some(10), None, None).await.unwrap();

    // Ignored partner entries:
    db.add_entry(user_id, "detailed", "2026-10-09 10:00", Some("Alleine"), Some("Emma"), None, Some(10), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-10 10:00", Some("Mit anderer Person"), None, None, Some(10), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-11 10:00", Some("Mit anderer Person"), Some(""), None, Some(10), None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-12 10:00", Some("Mit anderer Person"), Some("   "), None, Some(10), None, None).await.unwrap();

    let stats = db.get_statistics_at(user_id, now).await.unwrap();

    // Check Top Locations
    assert_eq!(stats.top_locations.len(), 3);
    assert_eq!(stats.top_locations[0].name, "Home");
    assert_eq!(stats.top_locations[0].count, 3);
    assert_eq!(stats.top_locations[0].avg_quality, Some(9.0));

    assert_eq!(stats.top_locations[1].name, "Hotel");
    assert_eq!(stats.top_locations[1].count, 2);
    assert_eq!(stats.top_locations[1].avg_quality, Some(10.0));

    assert_eq!(stats.top_locations[2].name, "Beach");
    assert_eq!(stats.top_locations[2].count, 2);
    assert_eq!(stats.top_locations[2].avg_quality, Some(7.0));

    // Check Top Partners
    assert_eq!(stats.top_partners.len(), 3);
    assert_eq!(stats.top_partners[0].name, "Julia");
    assert_eq!(stats.top_partners[0].count, 3);
    assert_eq!(stats.top_partners[0].avg_quality, Some(9.0));

    assert_eq!(stats.top_partners[1].name, "Sarah");
    assert_eq!(stats.top_partners[1].count, 2);
    assert_eq!(stats.top_partners[1].avg_quality, Some(10.0));

    assert_eq!(stats.top_partners[2].name, "Alex");
    assert_eq!(stats.top_partners[2].count, 2);
    assert_eq!(stats.top_partners[2].avg_quality, Some(8.0));
}

#[tokio::test]
async fn test_statistics_time_patterns_and_weekdays() {
    let (db, _tmp) = create_test_db().await;
    let tz = chrono_tz::Europe::Berlin;
    let now = chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 10, 20, 12, 0, 0).unwrap();
    let user_id = 500;

    // 2026-10-19 is a Monday
    // 2026-10-21 is a Wednesday

    // Night: 00:00 - 05:59 -> 02:30 (Monday)
    db.add_entry(user_id, "simple", "2026-10-19 02:30", None, None, None, None, None, None).await.unwrap();

    // Morning: 06:00 - 11:59 -> 08:15 (Monday)
    db.add_entry(user_id, "simple", "2026-10-19 08:15", None, None, None, None, None, None).await.unwrap();

    // Afternoon: 12:00 - 17:59 -> 14:00 (Wednesday)
    db.add_entry(user_id, "simple", "2026-10-21 14:00", None, None, None, None, None, None).await.unwrap();

    // Evening: 18:00 - 23:59 -> 20:45 (Wednesday)
    db.add_entry(user_id, "simple", "2026-10-21 20:45", None, None, None, None, None, None).await.unwrap();

    // Test RFC3339 timestamp parsing fallback: 2026-10-19 19:00:00 UTC = 2026-10-19 21:00:00 CEST (Monday, Evening)
    db.add_entry(user_id, "simple", "2026-10-19T19:00:00Z", None, None, None, None, None, None).await.unwrap();

    let stats = db.get_statistics_at(user_id, now).await.unwrap();

    assert_eq!(stats.time_of_day.night, 1);
    assert_eq!(stats.time_of_day.morning, 1);
    assert_eq!(stats.time_of_day.afternoon, 1);
    assert_eq!(stats.time_of_day.evening, 2);

    // Monday has 3 entries (02:30, 08:15, 21:00 Berlin). Wednesday has 2 entries (14:00, 20:45).
    assert_eq!(stats.most_active_weekday, Some(("Montag".to_string(), 3)));
}

#[tokio::test]
async fn test_statistics_weekday_tie_breaking() {
    let (db, _tmp) = create_test_db().await;
    let tz = chrono_tz::Europe::Berlin;
    let now = chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 10, 25, 12, 0, 0).unwrap();
    let user_id = 501;

    // Tie between Tuesday (2026-10-20) and Friday (2026-10-23): each 2 entries
    // Earlier day in the week (Dienstag) must win!
    db.add_entry(user_id, "simple", "2026-10-20 10:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_id, "simple", "2026-10-20 18:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_id, "simple", "2026-10-23 10:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_id, "simple", "2026-10-23 18:00", None, None, None, None, None, None).await.unwrap();

    let stats = db.get_statistics_at(user_id, now).await.unwrap();
    assert_eq!(stats.most_active_weekday, Some(("Dienstag".to_string(), 2)));
}

#[tokio::test]
async fn test_get_top_locations_aggregation_tiebreaker_and_limit() {
    let (db, _tmp) = create_test_db().await;
    let user_id = 1001;

    // Empty database returns empty
    assert!(db.get_top_locations(user_id, 4).await.unwrap().is_empty());

    // Null, empty, whitespace-only locations must be ignored
    db.add_entry(user_id, "detailed", "2026-10-01 10:00", None, None, None, None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-01 11:00", None, None, Some(""), None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-01 12:00", None, None, Some("   "), None, None, None).await.unwrap();

    // "Zuhause": 3 entries (whitespace trimmed and grouped)
    db.add_entry(user_id, "detailed", "2026-10-02 10:00", None, None, Some("Zuhause"), None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-03 10:00", None, None, Some("  Zuhause "), None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-04 10:00", None, None, Some("Zuhause"), None, None, None).await.unwrap();

    // "Hotel": 2 entries (older max timestamp: 2026-10-06)
    db.add_entry(user_id, "detailed", "2026-10-05 10:00", None, None, Some("Hotel"), None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-06 10:00", None, None, Some("Hotel"), None, None, None).await.unwrap();

    // "Strand": 2 entries (newer max timestamp: 2026-10-08 -> wins tiebreaker over Hotel)
    db.add_entry(user_id, "detailed", "2026-10-07 10:00", None, None, Some("Strand"), None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-08 10:00", None, None, Some("Strand"), None, None, None).await.unwrap();

    // "Auto": 1 entry (2026-10-09)
    db.add_entry(user_id, "detailed", "2026-10-09 10:00", None, None, Some("Auto"), None, None, None).await.unwrap();

    // "Wald": 1 entry (2026-10-10 -> wins tiebreaker over Auto)
    db.add_entry(user_id, "detailed", "2026-10-10 10:00", None, None, Some("Wald"), None, None, None).await.unwrap();

    let top4 = db.get_top_locations(user_id, 4).await.unwrap();
    assert_eq!(top4, vec!["Zuhause", "Strand", "Hotel", "Wald"]);

    let top2 = db.get_top_locations(user_id, 2).await.unwrap();
    assert_eq!(top2, vec!["Zuhause", "Strand"]);

    assert!(db.get_top_locations(user_id, 0).await.unwrap().is_empty());
    assert!(db.get_top_locations(user_id, -5).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_get_top_partners_aggregation_tiebreaker_and_limit() {
    let (db, _tmp) = create_test_db().await;
    let user_id = 1002;

    assert!(db.get_top_partners(user_id, 4).await.unwrap().is_empty());

    // Ignored: NULL, empty, whitespace, and company='Alleine'
    db.add_entry(user_id, "detailed", "2026-10-01 10:00", Some("Mit anderer Person"), None, None, None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-01 11:00", Some("Mit anderer Person"), Some(""), None, None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-01 12:00", Some("Mit anderer Person"), Some("   "), None, None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-01 13:00", Some("Alleine"), Some("Ignoriert"), None, None, None, None).await.unwrap();

    // "Julia": 3 entries
    db.add_entry(user_id, "detailed", "2026-10-02 10:00", Some("Mit anderer Person"), Some("Julia"), None, None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-03 10:00", Some("Mit anderer Person"), Some(" Julia "), None, None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-04 10:00", Some("Mit anderer Person"), Some("Julia"), None, None, None, None).await.unwrap();

    // "Alex": 2 entries (2026-10-05)
    db.add_entry(user_id, "detailed", "2026-10-05 09:00", Some("Mit anderer Person"), Some("Alex"), None, None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-05 10:00", Some("Mit anderer Person"), Some("Alex"), None, None, None, None).await.unwrap();

    // "Sam": 2 entries (2026-10-07 -> newer than Alex)
    db.add_entry(user_id, "detailed", "2026-10-06 10:00", Some("Mit anderer Person"), Some("Sam"), None, None, None, None).await.unwrap();
    db.add_entry(user_id, "detailed", "2026-10-07 10:00", Some("Mit anderer Person"), Some("Sam"), None, None, None, None).await.unwrap();

    // "Mika": 1 entry (2026-10-08)
    db.add_entry(user_id, "detailed", "2026-10-08 10:00", Some("Mit anderer Person"), Some("Mika"), None, None, None, None).await.unwrap();

    // "Lukas": 1 entry (2026-10-09 -> newer than Mika)
    db.add_entry(user_id, "detailed", "2026-10-09 10:00", Some("Mit anderer Person"), Some("Lukas"), None, None, None, None).await.unwrap();

    let top4 = db.get_top_partners(user_id, 4).await.unwrap();
    assert_eq!(top4, vec!["Julia", "Sam", "Alex", "Lukas"]);

    let top2 = db.get_top_partners(user_id, 2).await.unwrap();
    assert_eq!(top2, vec!["Julia", "Sam"]);

    assert!(db.get_top_partners(user_id, 0).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_top_suggestions_strict_multi_user_isolation() {
    let (db, _tmp) = create_test_db().await;
    let user_a = 11111;
    let user_b = 22222;
    let user_c = 33333;

    // User A records
    for _ in 0..3 {
        db.add_entry(user_a, "detailed", "2026-10-10 10:00", Some("Mit anderer Person"), Some("PartnerA1"), Some("OrtA1"), None, None, None).await.unwrap();
    }
    db.add_entry(user_a, "detailed", "2026-10-11 10:00", Some("Mit anderer Person"), Some("PartnerA2"), Some("OrtA2"), None, None, None).await.unwrap();

    // User B records (higher frequency so any missing WHERE user_id = ? would leak User B's items first)
    for _ in 0..10 {
        db.add_entry(user_b, "detailed", "2026-10-12 10:00", Some("Mit anderer Person"), Some("PartnerB1"), Some("OrtB1"), None, None, None).await.unwrap();
    }
    for _ in 0..5 {
        db.add_entry(user_b, "detailed", "2026-10-13 10:00", Some("Mit anderer Person"), Some("PartnerB2"), Some("OrtB2"), None, None, None).await.unwrap();
    }

    let locs_a = db.get_top_locations(user_a, 4).await.unwrap();
    assert_eq!(locs_a, vec!["OrtA1", "OrtA2"]);
    assert!(!locs_a.iter().any(|l| l.contains('B')));

    let partners_a = db.get_top_partners(user_a, 4).await.unwrap();
    assert_eq!(partners_a, vec!["PartnerA1", "PartnerA2"]);
    assert!(!partners_a.iter().any(|p| p.contains('B')));

    let locs_b = db.get_top_locations(user_b, 4).await.unwrap();
    assert_eq!(locs_b, vec!["OrtB1", "OrtB2"]);

    let partners_b = db.get_top_partners(user_b, 4).await.unwrap();
    assert_eq!(partners_b, vec!["PartnerB1", "PartnerB2"]);

    // User C has no entries
    assert!(db.get_top_locations(user_c, 4).await.unwrap().is_empty());
    assert!(db.get_top_partners(user_c, 4).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_non_destructive_migration_preserves_existing_records_and_creates_indices() {
    use sqlx::Row;

    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap().to_string();

    // Pre-populate an existing database without the new location/partner indices
    {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect(&format!("sqlite://{}", db_path))
            .await
            .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                company TEXT NULL,
                partner_name TEXT NULL,
                location TEXT NULL,
                quality INTEGER NULL,
                description TEXT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO entries (user_id, type, timestamp, company, partner_name, location, quality, description, created_at)
            VALUES
                (42, 'detailed', '2026-10-01 10:00', 'Mit anderer Person', 'Anna', 'Zuhause', 9, 'Alt 1', '2026-10-01T10:00:00Z'),
                (42, 'detailed', '2026-10-02 10:00', 'Mit anderer Person', 'Anna', 'Zuhause', 8, 'Alt 2', '2026-10-02T10:00:00Z'),
                (42, 'detailed', '2026-10-03 10:00', 'Alleine', NULL, 'Hotel', 7, 'Alt 3', '2026-10-03T10:00:00Z');
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        pool.close().await;
    }

    // Re-open with Database::connect() (calls init_db)
    let db = Database::connect(&db_path).await.unwrap();

    // Existing records must still be completely intact
    let all = db.get_all_entries(42).await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].description.as_deref(), Some("Alt 1"));
    assert_eq!(all[2].location.as_deref(), Some("Hotel"));

    // Suggestions work immediately on preserved records
    assert_eq!(db.get_top_locations(42, 4).await.unwrap(), vec!["Zuhause", "Hotel"]);
    assert_eq!(db.get_top_partners(42, 4).await.unwrap(), vec!["Anna"]);

    // Verify new indices were created non-destructively
    let idx_rows = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'entries';",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    let idx_names: Vec<String> = idx_rows.into_iter().map(|r| r.get("name")).collect();
    assert!(idx_names.contains(&"idx_entries_user_ts".to_string()));
    assert!(idx_names.contains(&"idx_entries_user_location".to_string()));
    assert!(idx_names.contains(&"idx_entries_user_partner".to_string()));
}

