// Goal tracking: a small, local SQLite store with an objective, a step
// budget and a status. A step records a line of progress and consumes one
// unit of budget. Nothing here calls a model. Ported from services/goal.rs,
// on the library database's goals and goal_steps tables.

import crypto from "node:crypto";
import { AppError } from "../core/errors.mjs";
import { get, all, run, tx } from "../core/db.mjs";

const nowSecs = () => Math.floor(Date.now() / 1000);

const rowToGoal = (row) => ({
  id: row.id,
  thread_id: row.thread_id,
  objective: row.objective,
  budget: row.budget,
  status: row.status,
  created_at: row.created_at,
});

const required = (id) => {
  const row = get(`SELECT * FROM goals WHERE id = ?`, String(id));
  if (!row) throw AppError.invalidInput(`There is no goal called ${id}.`);
  return rowToGoal(row);
};

const setState = (id, status) => {
  run(`UPDATE goals SET status = ? WHERE id = ?`, status, String(id));
  return { ...required(id), status };
};

/** Start a goal with a step budget. */
export function goalStart(threadId, objective, budget) {
  const clean = String(objective ?? "").trim();
  if (!clean) throw AppError.invalidInput("A goal needs an objective.");
  const budgetNum = Math.round(Number(budget) || 0);
  if (budgetNum < 0) throw AppError.invalidInput("A goal's budget cannot be negative.");
  const goal = {
    id: `goal-${Date.now()}-${crypto.randomInt(0, 1000)}`,
    thread_id: String(threadId ?? ""),
    objective: clean,
    budget: budgetNum,
    status: "active",
    created_at: nowSecs(),
  };
  run(
    `INSERT INTO goals (id, thread_id, objective, budget, status, created_at)
     VALUES (?, ?, ?, ?, ?, ?)`,
    goal.id, goal.thread_id, goal.objective, goal.budget, goal.status, goal.created_at,
  );
  return goal;
}

/** Read one goal's current state. null when the id is unknown. */
export function goalStatus(id) {
  const row = get(`SELECT * FROM goals WHERE id = ?`, String(id));
  return row ? rowToGoal(row) : null;
}

/** Record a step's progress text and consume one unit of budget. */
export function goalStep(id, progress) {
  return tx(() => {
    const goal = required(id);
    if (goal.status !== "active") {
      throw AppError.invalidInput("Only an active goal can take a step.");
    }
    if (goal.budget <= 0) {
      throw AppError.invalidInput("This goal has no steps left in its budget.");
    }
    const seq = (get(`SELECT COALESCE(MAX(seq), 0) + 1 AS n FROM goal_steps WHERE goal_id = ?`, goal.id)?.n) ?? 1;
    run(
      `INSERT INTO goal_steps (goal_id, seq, progress, created_at) VALUES (?, ?, ?, ?)`,
      goal.id, seq, String(progress ?? ""), nowSecs(),
    );
    const next = { ...goal, budget: goal.budget - 1 };
    run(`UPDATE goals SET budget = ? WHERE id = ?`, next.budget, goal.id);
    return next;
  });
}

/** Mark a goal completed. */
export function goalComplete(id) {
  const goal = required(id);
  if (goal.status === "paused") throw AppError.invalidInput("Resume a paused goal before completing it.");
  if (goal.status === "blocked") throw AppError.invalidInput("A blocked goal cannot be completed. Clear the blocker first.");
  if (goal.status === "completed") throw AppError.invalidInput("This goal is already completed.");
  if (goal.status === "abandoned") throw AppError.invalidInput("An abandoned goal cannot be completed.");
  return setState(id, "completed");
}

/** Pause a goal: progress stopped, but it is not done. */
export function goalPause(id) {
  const goal = required(id);
  if (goal.status !== "active") throw AppError.invalidInput("Only an active goal can be paused.");
  return setState(id, "paused");
}

/** Mark a goal blocked: it waits on something outside the run. */
export function goalBlock(id) {
  const goal = required(id);
  if (goal.status !== "active") throw AppError.invalidInput("Only an active goal can be marked blocked.");
  return setState(id, "blocked");
}

/** Resume a paused or blocked goal. */
export function goalResume(id) {
  const goal = required(id);
  if (goal.status === "active") throw AppError.invalidInput("This goal is already active.");
  if (goal.status === "completed") throw AppError.invalidInput("A completed goal cannot be resumed.");
  if (goal.status === "abandoned") throw AppError.invalidInput("An abandoned goal cannot be resumed.");
  return setState(id, "active");
}

/** Progress lines already recorded for a goal, oldest first. */
export function goalSteps(id) {
  if (!get(`SELECT id FROM goals WHERE id = ?`, String(id))) return [];
  return all(`SELECT * FROM goal_steps WHERE goal_id = ? ORDER BY seq`, String(id));
}
