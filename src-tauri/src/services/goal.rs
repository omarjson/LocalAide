//! Goal tracking: a small, local SQLite store under `app_dir()/index/goals.db`.
//!
//! A goal has an objective, a step budget and a status. A step records a line
//! of progress and consumes one unit of budget. Nothing here calls a model:
//! the caller decides what work a step means and passes its progress text in.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::app::error::AppError;

/// File name inside `app_dir()/index`.
pub const GOALS_DB: &str = "goals.db";

/// Where the goals database lives by default.
pub fn default_db_path() -> PathBuf {
    crate::app::paths::index_dir().join(GOALS_DB)
}

/// A goal's lifecycle state. Serialized as a lowercase string for the UI.
/// `Paused` is where a goal waits when progress stops (budget used up,
/// a failed step, the user stopping it, a rejected permission); `Blocked`
/// is where it waits on something outside the run. Either can resume.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    Paused,
    Blocked,
    Completed,
    Abandoned,
}

impl GoalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            GoalStatus::Active => "active",
            GoalStatus::Paused => "paused",
            GoalStatus::Blocked => "blocked",
            GoalStatus::Completed => "completed",
            GoalStatus::Abandoned => "abandoned",
        }
    }

    /// Unknown values are treated as active so a future status never strands
    /// an existing goal.
    pub fn parse(value: &str) -> Self {
        match value {
            "paused" => GoalStatus::Paused,
            "blocked" => GoalStatus::Blocked,
            "completed" => GoalStatus::Completed,
            "abandoned" => GoalStatus::Abandoned,
            _ => GoalStatus::Active,
        }
    }
}

/// One tracked goal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Goal {
    pub id: String,
    pub thread_id: String,
    pub objective: String,
    /// Steps still available. A step decrements it by one.
    pub budget: i64,
    pub status: GoalStatus,
    /// Unix timestamp, seconds since the epoch (UTC).
    pub created_at: i64,
}

/// One line of progress recorded by a step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoalStep {
    pub id: i64,
    pub goal_id: String,
    pub seq: i64,
    pub progress: String,
    pub created_at: i64,
}

/// Pure state transition for one step: consume one unit of budget.
pub fn apply_step(goal: &Goal) -> Result<Goal, AppError> {
    if goal.status != GoalStatus::Active {
        return Err(AppError::invalid_input(
            "Only an active goal can take a step.",
        ));
    }
    if goal.budget <= 0 {
        return Err(AppError::invalid_input(
            "This goal has no steps left in its budget.",
        ));
    }
    let mut next = goal.clone();
    next.budget -= 1;
    Ok(next)
}

/// Pure state transition for completion.
pub fn apply_complete(goal: &Goal) -> Result<Goal, AppError> {
    match goal.status {
        GoalStatus::Active => {}
        GoalStatus::Paused => {
            return Err(AppError::invalid_input(
                "Resume a paused goal before completing it.",
            ));
        }
        GoalStatus::Blocked => {
            return Err(AppError::invalid_input(
                "A blocked goal cannot be completed. Clear the blocker first.",
            ));
        }
        GoalStatus::Completed => {
            return Err(AppError::invalid_input("This goal is already completed."));
        }
        GoalStatus::Abandoned => {
            return Err(AppError::invalid_input(
                "An abandoned goal cannot be completed.",
            ));
        }
    }
    let mut next = goal.clone();
    next.status = GoalStatus::Completed;
    Ok(next)
}

/// Pure state transition for pausing: progress stopped, but the goal is
/// not done — budget used up, a step failed, the user stopped the run,
/// or a permission was rejected. Only an active goal can pause.
pub fn apply_pause(goal: &Goal) -> Result<Goal, AppError> {
    if goal.status != GoalStatus::Active {
        return Err(AppError::invalid_input(
            "Only an active goal can be paused.",
        ));
    }
    let mut next = goal.clone();
    next.status = GoalStatus::Paused;
    Ok(next)
}

/// Pure state transition for blocking: the goal waits on something
/// outside the run. Only an active goal can be marked blocked.
pub fn apply_block(goal: &Goal) -> Result<Goal, AppError> {
    if goal.status != GoalStatus::Active {
        return Err(AppError::invalid_input(
            "Only an active goal can be marked blocked.",
        ));
    }
    let mut next = goal.clone();
    next.status = GoalStatus::Blocked;
    Ok(next)
}

/// Pure state transition for resuming a paused or blocked goal.
pub fn apply_resume(goal: &Goal) -> Result<Goal, AppError> {
    match goal.status {
        GoalStatus::Paused | GoalStatus::Blocked => {}
        GoalStatus::Active => {
            return Err(AppError::invalid_input("This goal is already active."));
        }
        GoalStatus::Completed => {
            return Err(AppError::invalid_input(
                "A completed goal cannot be resumed.",
            ));
        }
        GoalStatus::Abandoned => {
            return Err(AppError::invalid_input(
                "An abandoned goal cannot be resumed.",
            ));
        }
    }
    let mut next = goal.clone();
    next.status = GoalStatus::Active;
    Ok(next)
}

/// Why a bounded goal loop stopped. `Blocked` is the independent check the
/// caller makes before deciding a goal is finished; `Paused` is where a
/// goal waits when progress stops without finishing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    BudgetExhausted,
    Blocked,
    Paused,
    Completed,
    Abandoned,
}

/// What the frontend should do on the next turn. On `Continue` it runs
/// `prompt` through the model, then feeds the result back with `goal_step`,
/// which consumes one unit of budget. This module never calls a model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepDecision {
    Continue { prompt: String, budget_left: i64 },
    Stop(StopReason),
}

/// Result of driving a goal over queued progress lines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriveResult {
    pub goal: Goal,
    pub applied: usize,
    pub decision: StepDecision,
}

/// Pure prompt builder for one step: the objective, the progress so far and
/// the remaining budget. No database, model or clock is involved.
pub fn step_prompt(goal: &Goal, steps: &[GoalStep]) -> String {
    let mut out = format!("Goal: {}\n", goal.objective);
    if steps.is_empty() {
        out.push_str("No progress recorded yet.\n");
    } else {
        out.push_str("Progress so far:\n");
        for step in steps {
            out.push_str(&format!("- {}\n", step.progress));
        }
    }
    out.push_str(&format!(
        "Steps left: {}. Take the next step, or say the goal is done.",
        goal.budget
    ));
    out
}

/// Decide the next action from a goal's state. A blocked report or a finished
/// goal stops the loop even when budget remains, so completion is never
/// inferred from budget alone. A paused or blocked goal stops too: it only
/// moves again after an explicit resume.
pub fn decide_next(goal: &Goal, steps: &[GoalStep], blocked: bool) -> StepDecision {
    match goal.status {
        GoalStatus::Completed => return StepDecision::Stop(StopReason::Completed),
        GoalStatus::Abandoned => return StepDecision::Stop(StopReason::Abandoned),
        GoalStatus::Paused => return StepDecision::Stop(StopReason::Paused),
        GoalStatus::Blocked => return StepDecision::Stop(StopReason::Blocked),
        GoalStatus::Active => {}
    }
    if blocked {
        return StepDecision::Stop(StopReason::Blocked);
    }
    if goal.budget <= 0 {
        return StepDecision::Stop(StopReason::BudgetExhausted);
    }
    StepDecision::Continue {
        prompt: step_prompt(goal, steps),
        budget_left: goal.budget,
    }
}

/// Bounded step loop. Applies at most one queued progress line per remaining
/// budget unit to a clone of the goal, stopping at once when the caller
/// reports a blocker. The loop is bounded by `queued.len()` and by `budget`,
/// so it cannot run forever, and it never touches the database: the caller
/// persists each accepted step with `step`/`goal_step`.
pub fn drive(goal: &Goal, queued: &[String], blocked: bool) -> DriveResult {
    let mut current = goal.clone();
    let mut applied = 0usize;
    let mut steps: Vec<GoalStep> = Vec::new();
    if !blocked {
        for progress in queued {
            match apply_step(&current) {
                Ok(next) => {
                    current = next;
                    applied += 1;
                    steps.push(GoalStep {
                        id: applied as i64,
                        goal_id: current.id.clone(),
                        seq: applied as i64,
                        progress: progress.clone(),
                        created_at: 0,
                    });
                }
                // Budget exhausted or the goal is no longer active.
                Err(_) => break,
            }
        }
    }
    let decision = if blocked {
        StepDecision::Stop(StopReason::Blocked)
    } else {
        decide_next(&current, &steps, false)
    };
    // The state machine moves on its own when progress stops: a blocked
    // turn waits as Blocked, a used-up budget waits as Paused. Only an
    // active goal moves — a finished one keeps its state. The caller
    // persists the returned goal; nothing here touches the database.
    if current.status == GoalStatus::Active {
        if blocked {
            current.status = GoalStatus::Blocked;
        } else if decision == StepDecision::Stop(StopReason::BudgetExhausted) {
            current.status = GoalStatus::Paused;
        }
    }
    DriveResult {
        goal: current,
        applied,
        decision,
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A dependency-free unique id: millisecond clock plus a process-local
/// counter, so two goals started in the same tick still differ.
fn new_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("goal-{millis}-{n}")
}

fn open(path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::storage(format!("Could not create the index folder: {e}")))?;
    }
    let conn = Connection::open(path)
        .map_err(|e| AppError::storage(format!("Could not open the goals database: {e}")))?;
    conn.execute_batch("PRAGMA busy_timeout = 5000;")
        .map_err(|e| AppError::storage(format!("Could not configure the goals database: {e}")))?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// Idempotent schema: both tables and the index are created only if missing.
fn ensure_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS goals (
             id TEXT PRIMARY KEY,
             thread_id TEXT NOT NULL,
             objective TEXT NOT NULL,
             budget INTEGER NOT NULL,
             status TEXT NOT NULL,
             created_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS goal_steps (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             goal_id TEXT NOT NULL,
             seq INTEGER NOT NULL,
             progress TEXT NOT NULL,
             created_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS goal_steps_goal ON goal_steps(goal_id, seq);",
    )
    .map_err(|e| AppError::storage(format!("Could not create the goals schema: {e}")))
}

fn row_to_goal(row: &rusqlite::Row<'_>) -> rusqlite::Result<Goal> {
    Ok(Goal {
        id: row.get(0)?,
        thread_id: row.get(1)?,
        objective: row.get(2)?,
        budget: row.get(3)?,
        status: GoalStatus::parse(&row.get::<_, String>(4)?),
        created_at: row.get(5)?,
    })
}

fn load(conn: &Connection, id: &str) -> Result<Option<Goal>, AppError> {
    conn.query_row(
        "SELECT id, thread_id, objective, budget, status, created_at FROM goals WHERE id = ?1",
        params![id],
        row_to_goal,
    )
    .optional()
    .map_err(|e| AppError::storage(format!("Could not read the goal: {e}")))
}

fn required(conn: &Connection, id: &str) -> Result<Goal, AppError> {
    load(conn, id)?.ok_or_else(|| AppError::invalid_input(format!("There is no goal called {id}.")))
}

/// Start a goal. `budget` is the number of steps it may take.
pub fn start_at(
    path: &Path,
    thread_id: &str,
    objective: &str,
    budget: i64,
) -> Result<Goal, AppError> {
    if objective.trim().is_empty() {
        return Err(AppError::invalid_input("A goal needs an objective."));
    }
    if budget < 0 {
        return Err(AppError::invalid_input(
            "A goal's budget cannot be negative.",
        ));
    }
    let goal = Goal {
        id: new_id(),
        thread_id: thread_id.to_string(),
        objective: objective.trim().to_string(),
        budget,
        status: GoalStatus::Active,
        created_at: now_secs(),
    };
    let conn = open(path)?;
    conn.execute(
        "INSERT INTO goals(id, thread_id, objective, budget, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            goal.id,
            goal.thread_id,
            goal.objective,
            goal.budget,
            goal.status.as_str(),
            goal.created_at
        ],
    )
    .map_err(|e| AppError::storage(format!("Could not save the goal: {e}")))?;
    Ok(goal)
}

/// Start a goal in the default database.
pub fn start(thread_id: &str, objective: &str, budget: i64) -> Result<Goal, AppError> {
    start_at(&default_db_path(), thread_id, objective, budget)
}

/// Current state of one goal; `None` when the id is unknown.
pub fn status_at(path: &Path, id: &str) -> Result<Option<Goal>, AppError> {
    let conn = open(path)?;
    load(&conn, id)
}

/// Current state of one goal in the default database.
pub fn status(id: &str) -> Result<Option<Goal>, AppError> {
    status_at(&default_db_path(), id)
}

/// All goals, optionally restricted to one chat thread, newest first.
pub fn list_at(path: &Path, thread_id: Option<&str>) -> Result<Vec<Goal>, AppError> {
    let conn = open(path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, thread_id, objective, budget, status, created_at FROM goals
             WHERE ?1 IS NULL OR thread_id = ?1
             ORDER BY created_at DESC, id DESC",
        )
        .map_err(|e| AppError::storage(format!("Could not prepare the goal list: {e}")))?;
    let rows = stmt
        .query_map(params![thread_id], row_to_goal)
        .map_err(|e| AppError::storage(format!("Could not list goals: {e}")))?;
    let mut goals = Vec::new();
    for row in rows {
        goals.push(row.map_err(|e| AppError::storage(format!("Could not list goals: {e}")))?);
    }
    Ok(goals)
}

/// All goals in the default database.
pub fn list(thread_id: Option<&str>) -> Result<Vec<Goal>, AppError> {
    list_at(&default_db_path(), thread_id)
}

/// Record a step's progress text and decrement the budget, atomically.
pub fn step_at(path: &Path, id: &str, progress: &str) -> Result<Goal, AppError> {
    let mut conn = open(path)?;
    let tx = conn
        .transaction()
        .map_err(|e| AppError::storage(format!("Could not start the goal step: {e}")))?;
    let goal = required(&tx, id)?;
    let next = apply_step(&goal)?;
    let seq: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM goal_steps WHERE goal_id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| AppError::storage(format!("Could not number the goal step: {e}")))?;
    tx.execute(
        "INSERT INTO goal_steps(goal_id, seq, progress, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, seq, progress, now_secs()],
    )
    .map_err(|e| AppError::storage(format!("Could not record the goal step: {e}")))?;
    tx.execute(
        "UPDATE goals SET budget = ?1 WHERE id = ?2",
        params![next.budget, id],
    )
    .map_err(|e| AppError::storage(format!("Could not update the goal budget: {e}")))?;
    tx.commit()
        .map_err(|e| AppError::storage(format!("Could not commit the goal step: {e}")))?;
    Ok(next)
}

/// Record a step in the default database.
pub fn step(id: &str, progress: &str) -> Result<Goal, AppError> {
    step_at(&default_db_path(), id, progress)
}

/// Progress lines already recorded for a goal, oldest first.
pub fn steps_at(path: &Path, id: &str) -> Result<Vec<GoalStep>, AppError> {
    let conn = open(path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, goal_id, seq, progress, created_at FROM goal_steps
             WHERE goal_id = ?1 ORDER BY seq",
        )
        .map_err(|e| AppError::storage(format!("Could not prepare the goal steps: {e}")))?;
    let rows = stmt
        .query_map(params![id], |row| {
            Ok(GoalStep {
                id: row.get(0)?,
                goal_id: row.get(1)?,
                seq: row.get(2)?,
                progress: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| AppError::storage(format!("Could not list goal steps: {e}")))?;
    let mut steps = Vec::new();
    for row in rows {
        steps.push(row.map_err(|e| AppError::storage(format!("Could not list goal steps: {e}")))?);
    }
    Ok(steps)
}

/// Progress lines in the default database.
pub fn steps(id: &str) -> Result<Vec<GoalStep>, AppError> {
    steps_at(&default_db_path(), id)
}

/// Mark a goal completed.
pub fn complete_at(path: &Path, id: &str) -> Result<Goal, AppError> {
    let conn = open(path)?;
    let goal = required(&conn, id)?;
    let next = apply_complete(&goal)?;
    conn.execute(
        "UPDATE goals SET status = ?1 WHERE id = ?2",
        params![next.status.as_str(), id],
    )
    .map_err(|e| AppError::storage(format!("Could not complete the goal: {e}")))?;
    Ok(next)
}

/// Mark a goal completed in the default database.
pub fn complete(id: &str) -> Result<Goal, AppError> {
    complete_at(&default_db_path(), id)
}

/// Move a goal to a new status with a pure transition, atomically.
fn set_status_at(
    path: &Path,
    id: &str,
    next: fn(&Goal) -> Result<Goal, AppError>,
) -> Result<Goal, AppError> {
    let conn = open(path)?;
    let goal = required(&conn, id)?;
    let next = next(&goal)?;
    conn.execute(
        "UPDATE goals SET status = ?1 WHERE id = ?2",
        params![next.status.as_str(), id],
    )
    .map_err(|e| AppError::storage(format!("Could not update the goal: {e}")))?;
    Ok(next)
}

/// Pause a goal in the given database: progress stopped, but it is not done.
pub fn pause_at(path: &Path, id: &str) -> Result<Goal, AppError> {
    set_status_at(path, id, apply_pause)
}

/// Pause a goal in the default database.
pub fn pause(id: &str) -> Result<Goal, AppError> {
    pause_at(&default_db_path(), id)
}

/// Mark a goal blocked in the given database: it waits on something
/// outside the run.
pub fn block_at(path: &Path, id: &str) -> Result<Goal, AppError> {
    set_status_at(path, id, apply_block)
}

/// Mark a goal blocked in the default database.
pub fn block(id: &str) -> Result<Goal, AppError> {
    block_at(&default_db_path(), id)
}

/// Resume a paused or blocked goal in the given database.
pub fn resume_at(path: &Path, id: &str) -> Result<Goal, AppError> {
    set_status_at(path, id, apply_resume)
}

/// Resume a paused or blocked goal in the default database.
pub fn resume(id: &str) -> Result<Goal, AppError> {
    resume_at(&default_db_path(), id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn goal(budget: i64, status: GoalStatus) -> Goal {
        Goal {
            id: "g1".into(),
            thread_id: "t1".into(),
            objective: "ship it".into(),
            budget,
            status,
            created_at: 1,
        }
    }

    #[test]
    fn a_step_consumes_one_unit_of_budget() {
        let first = apply_step(&goal(3, GoalStatus::Active)).unwrap();
        assert_eq!(first.budget, 2);
        assert_eq!(first.status, GoalStatus::Active);
        let second = apply_step(&first).unwrap();
        assert_eq!(second.budget, 1);
    }

    #[test]
    fn a_step_is_refused_without_budget_or_on_a_finished_goal() {
        assert!(apply_step(&goal(0, GoalStatus::Active)).is_err());
        assert!(apply_step(&goal(2, GoalStatus::Completed)).is_err());
        assert!(apply_step(&goal(2, GoalStatus::Abandoned)).is_err());
    }

    #[test]
    fn completion_is_a_one_way_transition() {
        let done = apply_complete(&goal(2, GoalStatus::Active)).unwrap();
        assert_eq!(done.status, GoalStatus::Completed);
        assert_eq!(done.budget, 2);
        assert!(apply_complete(&done).is_err());
        assert!(apply_step(&done).is_err());
    }

    #[test]
    fn drive_stops_at_budget() {
        let start = goal(2, GoalStatus::Active);
        let queued = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let result = drive(&start, &queued, false);
        assert_eq!(result.applied, 2, "no more steps than budget");
        assert_eq!(result.goal.budget, 0);
        assert_eq!(
            result.decision,
            StepDecision::Stop(StopReason::BudgetExhausted)
        );
    }

    #[test]
    fn drive_stops_when_blocked() {
        let start = goal(3, GoalStatus::Active);
        let queued = vec!["a".to_string(), "b".to_string()];
        let result = drive(&start, &queued, true);
        assert_eq!(result.applied, 0);
        assert_eq!(result.goal.budget, 3, "a blocked turn spends no budget");
        assert_eq!(result.decision, StepDecision::Stop(StopReason::Blocked));
    }

    #[test]
    fn decide_next_offers_a_prompt_while_work_remains() {
        let start = goal(1, GoalStatus::Active);
        let steps = vec![GoalStep {
            id: 1,
            goal_id: start.id.clone(),
            seq: 1,
            progress: "drafted the outline".into(),
            created_at: 0,
        }];
        match decide_next(&start, &steps, false) {
            StepDecision::Continue {
                prompt,
                budget_left,
            } => {
                assert_eq!(budget_left, 1);
                assert!(prompt.contains("ship it"), "{prompt}");
                assert!(prompt.contains("drafted the outline"), "{prompt}");
            }
            other => panic!("expected a prompt, got {other:?}"),
        }
        assert_eq!(
            decide_next(&start, &steps, true),
            StepDecision::Stop(StopReason::Blocked)
        );
        let done = goal(1, GoalStatus::Completed);
        assert_eq!(
            decide_next(&done, &[], false),
            StepDecision::Stop(StopReason::Completed)
        );
    }

    #[test]
    fn status_strings_round_trip() {
        for status in [
            GoalStatus::Active,
            GoalStatus::Paused,
            GoalStatus::Blocked,
            GoalStatus::Completed,
            GoalStatus::Abandoned,
        ] {
            assert_eq!(GoalStatus::parse(status.as_str()), status);
        }
        assert_eq!(GoalStatus::parse("something-new"), GoalStatus::Active);
    }

    #[test]
    fn pause_block_and_resume_are_one_way_out_of_active() {
        let active = goal(2, GoalStatus::Active);
        let paused = apply_pause(&active).unwrap();
        assert_eq!(paused.status, GoalStatus::Paused);
        assert_eq!(paused.budget, 2, "pausing spends no budget");
        assert!(apply_step(&paused).is_err(), "a paused goal takes no steps");
        assert!(apply_complete(&paused).is_err());
        assert!(apply_pause(&paused).is_err());
        let resumed = apply_resume(&paused).unwrap();
        assert_eq!(resumed.status, GoalStatus::Active);
        assert!(apply_resume(&resumed).is_err());

        let blocked = apply_block(&active).unwrap();
        assert_eq!(blocked.status, GoalStatus::Blocked);
        assert!(apply_step(&blocked).is_err());
        assert_eq!(
            decide_next(&blocked, &[], false),
            StepDecision::Stop(StopReason::Blocked)
        );
        assert_eq!(
            decide_next(&paused, &[], false),
            StepDecision::Stop(StopReason::Paused)
        );
    }

    #[test]
    fn drive_auto_pauses_on_budget_and_blocks_when_blocked() {
        let start = goal(1, GoalStatus::Active);
        let result = drive(&start, &["a".to_string(), "b".to_string()], false);
        assert_eq!(result.applied, 1);
        assert_eq!(
            result.decision,
            StepDecision::Stop(StopReason::BudgetExhausted)
        );
        assert_eq!(result.goal.status, GoalStatus::Paused);

        let blocked = drive(&start, &["a".to_string()], true);
        assert_eq!(blocked.applied, 0);
        assert_eq!(blocked.goal.status, GoalStatus::Blocked);

        // A finished goal keeps its state when driven again.
        let done = goal(0, GoalStatus::Completed);
        assert_eq!(
            drive(&done, &["a".to_string()], true).goal.status,
            GoalStatus::Completed
        );
    }

    #[test]
    fn goals_and_steps_survive_a_round_trip() {
        let dir = std::env::temp_dir().join(format!("localaide-goals-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let db = dir.join("goals.db");

        let started = start_at(&db, "thread-a", "finish the report", 2).unwrap();
        assert_eq!(started.status, GoalStatus::Active);
        assert_eq!(started.budget, 2);
        // Reopening applies the same schema again without error.
        assert_eq!(status_at(&db, &started.id).unwrap(), Some(started.clone()));

        let after_one = step_at(&db, &started.id, "drafted the outline").unwrap();
        assert_eq!(after_one.budget, 1);
        let after_two = step_at(&db, &started.id, "filled in the numbers").unwrap();
        assert_eq!(after_two.budget, 0);
        assert!(step_at(&db, &started.id, "one too many").is_err());

        let saved = steps_at(&db, &started.id).unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].progress, "drafted the outline");
        assert_eq!(saved[1].progress, "filled in the numbers");
        assert_eq!(saved[0].seq, 1);
        assert_eq!(saved[1].seq, 2);

        let done = {
            // Pause, block and resume all persist: reopening the database
            // shows the same status, so goals survive restarts.
            let paused = pause_at(&db, &started.id).unwrap();
            assert_eq!(paused.status, GoalStatus::Paused);
            assert_eq!(
                status_at(&db, &started.id).unwrap().unwrap().status,
                GoalStatus::Paused
            );
            assert_eq!(
                resume_at(&db, &started.id).unwrap().status,
                GoalStatus::Active
            );
            assert_eq!(
                block_at(&db, &started.id).unwrap().status,
                GoalStatus::Blocked
            );
            assert_eq!(
                resume_at(&db, &started.id).unwrap().status,
                GoalStatus::Active
            );
            complete_at(&db, &started.id).unwrap()
        };
        assert_eq!(done.status, GoalStatus::Completed);
        assert!(complete_at(&db, &started.id).is_err());

        let all = list_at(&db, Some("thread-a")).unwrap();
        assert_eq!(all.len(), 1);
        assert!(list_at(&db, Some("other-thread")).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
