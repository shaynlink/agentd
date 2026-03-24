use std::path::PathBuf;

use agentd::adapters::store::sqlite::SqliteStore;
use agentd::app::App;
use agentd::domain::schedule::ScheduleState;
use agentd::ports::store::StateStore;
use chrono::{Duration, Utc};
use uuid::Uuid;

fn temp_db_path() -> String {
    let mut path = PathBuf::from(std::env::temp_dir());
    path.push(format!("agentd-test-{}.db", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

#[tokio::test]
async fn dispatch_due_run_at_schedule_executes_once() {
    let db_path = temp_db_path();
    let app = App::new(db_path.clone()).expect("create app");

    let run_at = Utc::now() - Duration::seconds(2);
    app.schedule_run_at("once", "mock", "do work", run_at, 10, 0, None)
        .expect("create run-at schedule");

    app.dispatch_due_schedules(50)
        .await
        .expect("dispatch due schedules");

    let store = SqliteStore::new(db_path);
    let schedules = store.list_schedules(10).expect("list schedules");
    assert_eq!(schedules.len(), 1, "expected one schedule");
    assert_eq!(schedules[0].state, ScheduleState::Succeeded);
}

#[tokio::test]
async fn dispatch_due_cron_schedule_replans_next_run() {
    let db_path = temp_db_path();
    let app = App::new(db_path.clone()).expect("create app");

    app.schedule_cron(
        "hourly",
        "mock",
        "do recurring work",
        "0 0 * * * * *",
        10,
        0,        None,    )
    .expect("create cron schedule");

    let store = SqliteStore::new(db_path.clone());
    let before = store
        .list_schedules(10)
        .expect("list schedules before dispatch");
    assert_eq!(before.len(), 1, "expected one schedule");

    let schedule_id = before[0].id.clone();
    let forced_due = Utc::now() - Duration::seconds(2);
    store
        .update_schedule_run_at(&schedule_id, &forced_due.to_rfc3339())
        .expect("force schedule as due");

    app.dispatch_due_schedules(50)
        .await
        .expect("dispatch due schedules");

    let after = store
        .list_schedules(10)
        .expect("list schedules after dispatch");
    assert_eq!(after.len(), 1, "expected one schedule");
    assert_eq!(after[0].state, ScheduleState::Scheduled);
    assert!(
        after[0].run_at > forced_due,
        "cron schedule should be re-planned after due timestamp"
    );
}
