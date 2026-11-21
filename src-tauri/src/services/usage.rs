//! A tiny usage log: one line per finished answer, aggregated for the
//! Usage screen. Local-only, plain JSONL next to the other app data, and
//! never allowed to break a chat — write failures are dropped silently.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::app::error::AppError;
use crate::app::paths;

/// One finished answer, recorded when a chat (or shared-link) run ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// When the answer finished, seconds since the epoch (UTC).
    pub ts: u64,
    /// Model id, as the picker shows it.
    pub model: String,
    /// Provider label: "This computer" or the online provider's name.
    pub provider: String,
    /// True when the model ran on this machine.
    pub local: bool,
    /// Estimated prompt size (chars / 4): servers rarely report it.
    pub input_tokens: usize,
    /// Tokens the answer produced (usage block when offered, else pieces).
    pub output_tokens: usize,
    /// Wall time of the whole answer, including tool rounds.
    pub ms: u64,
    /// Time to the first token of the first round, when known.
    pub ttft_ms: Option<u64>,
    /// Output tokens per second over the whole run, when measurable.
    pub tok_s: Option<f64>,
    pub cancelled: bool,
    pub tool_calls: usize,
    /// "chat" or "share".
    pub source: String,
    /// Project folder the chat worked in, when one was picked.
    #[serde(default)]
    pub folder: Option<String>,
    /// The chat thread this answer belongs to, when the caller knows it.
    /// Distinct ids make sessions; None counts as its own session.
    #[serde(default)]
    pub session: Option<String>,
    /// Cached prompt tokens the provider reported, when it does.
    #[serde(default)]
    pub cache_tokens: Option<usize>,
}

/// Answers and tokens on one UTC day.
#[derive(Debug, Serialize, PartialEq)]
pub struct UsageDay {
    /// "YYYY-MM-DD".
    pub date: String,
    pub answers: usize,
    pub input: usize,
    pub output: usize,
}

/// Per-model rollup over the requested window.
#[derive(Debug, Serialize, PartialEq)]
pub struct UsageModel {
    pub model: String,
    pub provider: String,
    pub answers: usize,
    pub input: usize,
    pub output: usize,
    /// Cached prompt tokens the model reported, summed.
    pub cache: usize,
    /// Tool runs the model made, summed.
    pub steps: usize,
    /// Mean wall time per answer.
    pub avg_ms: u64,
    /// Mean output tokens per second across answers that report speed.
    pub avg_tok_s: Option<f64>,
    /// Mean time to the first token across answers that report it.
    pub avg_ttft_ms: Option<u64>,
}

/// What the Usage screen renders.
#[derive(Debug, Serialize, PartialEq)]
pub struct UsageSummary {
    pub days: u32,
    pub answers: usize,
    pub cancelled: usize,
    pub input_tokens: usize,
    pub output_tokens: usize,
    /// Total wall time of all answers, milliseconds.
    pub ms: u64,
    pub tool_calls: usize,
    /// The window's days that had at least one answer, oldest first.
    pub per_day: Vec<UsageDay>,
    /// Busiest models first.
    pub per_model: Vec<UsageModel>,
    /// Cached prompt tokens the providers reported, summed.
    pub cache_tokens: usize,
    /// Distinct chat threads answered in the window. Records without a
    /// thread id each count as one session.
    pub sessions: usize,
    /// Days in the window that had at least one answer.
    pub active_days: usize,
    /// Longest run of consecutive active days, ending anywhere in the window.
    pub longest_streak: usize,
    /// Per-project rollup over the window, busiest first. Always the whole
    /// window, even when the summary is filtered to one project, so the
    /// picker keeps every project listed.
    pub per_folder: Vec<UsageProject>,
}

/// Per-project rollup over the window.
#[derive(Debug, Serialize, PartialEq)]
pub struct UsageProject {
    /// The project folder path, as the chat worked in it.
    pub folder: String,
    pub answers: usize,
    pub input: usize,
    pub output: usize,
    /// Total wall time, milliseconds.
    pub ms: u64,
}

pub fn log_path() -> PathBuf {
    paths::app_dir().join("usage.jsonl")
}

/// Appends one record. Never panics, never errors the caller: usage
/// tracking must not be able to break an answer.
pub fn record(rec: &UsageRecord) {
    let dir = paths::app_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    else {
        return;
    };
    if let Ok(line) = serde_json::to_string(rec) {
        let _ = writeln!(file, "{line}");
    }
}

/// Aggregates the log over a window and, when set, only the records of one
/// project folder. Windows: `1` is today (the current UTC day), `0` is all
/// time, anything else the rolling last `days` days.
pub fn summary(days: u32, folder: Option<&str>) -> Result<UsageSummary, AppError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    summary_at(&log_path(), days, now, folder)
}

/// Testable core: aggregates the file as if "now" were `now_secs`.
fn summary_at(
    path: &Path,
    days: u32,
    now_secs: u64,
    folder: Option<&str>,
) -> Result<UsageSummary, AppError> {
    let cutoff = match days {
        0 => 0,
        // Today runs from midnight UTC, not a rolling 24 hours.
        1 => now_secs.saturating_sub(now_secs % 86400),
        d => now_secs.saturating_sub(d as u64 * 86400),
    };
    let mut answers = 0usize;
    let mut cancelled = 0usize;
    let mut input = 0usize;
    let mut output = 0usize;
    let mut cache = 0usize;
    let mut ms = 0u64;
    let mut tool_calls = 0usize;
    let mut per_day: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    let mut per_model: BTreeMap<(String, String), ModelAcc> = BTreeMap::new();
    let mut per_folder: BTreeMap<String, UsageProject> = BTreeMap::new();
    let mut sessions: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut sessionless = 0usize;

    if let Ok(file) = File::open(path) {
        for line in BufReader::new(file).lines() {
            let Ok(line) = line else { continue };
            let Ok(rec) = serde_json::from_str::<UsageRecord>(line.trim()) else {
                continue;
            };
            if rec.ts < cutoff {
                continue;
            }
            // Projects count in their own rollup even while the summary is
            // filtered to another project, so the picker stays complete.
            if let Some(f) = &rec.folder {
                let proj = per_folder.entry(f.clone()).or_insert(UsageProject {
                    folder: f.clone(),
                    answers: 0,
                    input: 0,
                    output: 0,
                    ms: 0,
                });
                proj.answers += 1;
                proj.input += rec.input_tokens;
                proj.output += rec.output_tokens;
                proj.ms += rec.ms;
            }
            if let Some(wanted) = folder
                && rec.folder.as_deref() != Some(wanted)
            {
                continue;
            }
            answers += 1;
            if rec.cancelled {
                cancelled += 1;
            }
            input += rec.input_tokens;
            output += rec.output_tokens;
            cache += rec.cache_tokens.unwrap_or(0);
            ms += rec.ms;
            tool_calls += rec.tool_calls;
            match &rec.session {
                Some(id) => {
                    sessions.insert(id.clone());
                }
                None => sessionless += 1,
            }
            let day = per_day.entry(date_from_ts(rec.ts)).or_default();
            day.0 += 1;
            day.1 += rec.input_tokens;
            day.2 += rec.output_tokens;
            let acc = per_model.entry((rec.model, rec.provider)).or_default();
            acc.answers += 1;
            acc.input += rec.input_tokens;
            acc.output += rec.output_tokens;
            acc.cache += rec.cache_tokens.unwrap_or(0);
            acc.steps += rec.tool_calls;
            acc.ms += rec.ms;
            acc.ms_n += 1;
            if let Some(s) = rec.tok_s {
                acc.tok_s_sum += s;
                acc.tok_s_n += 1;
            }
            if let Some(t) = rec.ttft_ms {
                acc.ttft_sum += t;
                acc.ttft_n += 1;
            }
        }
    }

    // Longest run of consecutive active days. The days sort in BTreeMap
    // order (YYYY-MM-DD), so consecutive entries differ by one calendar day.
    let mut longest = 0usize;
    let mut run = 0usize;
    let mut prev: Option<i64> = None;
    for day in per_day.keys() {
        let stamp = match day_from_date(day) {
            Some(d) => d,
            None => continue,
        };
        run = match prev {
            Some(p) if stamp == p + 1 => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        prev = Some(stamp);
    }

    let per_day: Vec<UsageDay> = per_day
        .into_iter()
        .map(|(date, (d_answers, d_input, d_output))| UsageDay {
            date,
            answers: d_answers,
            input: d_input,
            output: d_output,
        })
        .collect();
    let mut per_model: Vec<UsageModel> = per_model
        .into_iter()
        .map(|((model, provider), acc)| UsageModel {
            avg_ms: if acc.ms_n > 0 {
                acc.ms / acc.ms_n as u64
            } else {
                0
            },
            avg_tok_s: if acc.tok_s_n > 0 {
                Some(acc.tok_s_sum / acc.tok_s_n as f64)
            } else {
                None
            },
            avg_ttft_ms: if acc.ttft_n > 0 {
                Some(acc.ttft_sum / acc.ttft_n as u64)
            } else {
                None
            },
            model,
            provider,
            answers: acc.answers,
            input: acc.input,
            output: acc.output,
            cache: acc.cache,
            steps: acc.steps,
        })
        .collect();
    per_model.sort_by(|a, b| b.output.cmp(&a.output).then(b.answers.cmp(&a.answers)));
    let mut per_folder: Vec<UsageProject> = per_folder.into_values().collect();
    per_folder.sort_by(|a, b| b.output.cmp(&a.output).then(b.answers.cmp(&a.answers)));

    Ok(UsageSummary {
        days,
        answers,
        cancelled,
        input_tokens: input,
        output_tokens: output,
        cache_tokens: cache,
        ms,
        tool_calls,
        sessions: sessions.len() + sessionless,
        active_days: per_day.len(),
        longest_streak: longest,
        per_day,
        per_model,
        per_folder,
    })
}

/// Days since the Unix epoch for a "YYYY-MM-DD" date, the inverse of
/// date_from_ts's format. None when the string is not a date.
fn day_from_date(date: &str) -> Option<i64> {
    let (y, rest) = date.split_once('-')?;
    let (m, d) = rest.split_once('-')?;
    let y: i64 = y.parse().ok()?;
    let m: i64 = m.parse().ok()?;
    let d: i64 = d.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y_adj = if m <= 2 { y - 1 } else { y };
    let era = y_adj.div_euclid(400);
    let yoe = y_adj - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

#[derive(Default)]
struct ModelAcc {
    answers: usize,
    input: usize,
    output: usize,
    cache: usize,
    steps: usize,
    ms: u64,
    ms_n: usize,
    tok_s_sum: f64,
    tok_s_n: usize,
    ttft_sum: u64,
    ttft_n: usize,
}

/// UTC calendar date for a Unix timestamp, no calendar crate:
/// Howard Hinnant's civil_from_days.
fn date_from_ts(secs: u64) -> String {
    let z = (secs / 86400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(ts: u64, model: &str, output: usize) -> UsageRecord {
        UsageRecord {
            ts,
            model: model.to_string(),
            provider: if model == "local-model" {
                "This computer".to_string()
            } else {
                "Kilo".to_string()
            },
            local: model == "local-model",
            input_tokens: 100,
            output_tokens: output,
            ms: 4_000,
            ttft_ms: Some(600),
            tok_s: Some(24.0),
            cancelled: false,
            tool_calls: 1,
            source: "chat".to_string(),
            folder: None,
            session: None,
            cache_tokens: None,
        }
    }

    fn rec_in(ts: u64, output: usize, folder: Option<&str>) -> UsageRecord {
        let mut r = rec(ts, "local-model", output);
        r.folder = folder.map(|f| f.to_string());
        r
    }

    /// A unique temp dir per test run, so parallel tests never share a log.
    fn temp_dir_for(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("localaide-usage-{name}-{}", std::process::id()))
    }

    fn write_log(path: &PathBuf, records: &[UsageRecord]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = File::create(path).unwrap();
        for r in records {
            writeln!(file, "{}", serde_json::to_string(r).unwrap()).unwrap();
        }
    }

    #[test]
    fn summary_aggregates_days_and_models() {
        let dir = temp_dir_for("base");
        let path = dir.join("usage.jsonl");
        let now = 1_800_000_000; // fixed "now" so the test never ages out
        write_log(
            &path,
            &[
                rec(now, "local-model", 200),
                rec(now - 3600, "qwen3-4b", 100),
                rec(now - 2 * 86400, "local-model", 50),
                // Outside a 1-day window, inside a 7-day window.
                rec(now - 3 * 86400, "local-model", 700),
                // Broken lines and a stale record are skipped, not errors.
                rec(now - 400 * 86400, "ancient", 9_999),
            ],
        );
        // Corrupt lines round the log out.
        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            writeln!(file, "not json").unwrap();
            writeln!(file, "{{oops").unwrap();
        }

        let one = summary_at(&path, 1, now, None).unwrap();
        assert_eq!(one.answers, 2);
        assert_eq!(one.output_tokens, 300);
        assert_eq!(one.per_day.len(), 1);

        let week = summary_at(&path, 7, now, None).unwrap();
        assert_eq!(week.answers, 4);
        assert_eq!(week.output_tokens, 1050);
        assert_eq!(week.per_day.len(), 3);
        // Busiest model first by output tokens.
        assert_eq!(week.per_model[0].model, "local-model");
        assert_eq!(week.per_model[0].answers, 3);
        assert_eq!(week.per_model[1].model, "qwen3-4b");
        assert_eq!(week.per_model[1].provider, "Kilo");
        assert_eq!(week.per_model[0].avg_tok_s, Some(24.0));
        assert_eq!(week.per_model[0].avg_ttft_ms, Some(600));

        let empty = summary_at(&dir.join("missing.jsonl"), 30, now, None).unwrap();
        assert_eq!(empty.answers, 0);
        assert!(empty.per_model.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn today_starts_at_utc_midnight() {
        let dir = temp_dir_for("today");
        let path = dir.join("usage.jsonl");
        // 12:30 UTC today, and 23:00 yesterday: "today" sees only the first.
        let now = 86_400 + 45_000; // day 2, 12:30
        write_log(
            &path,
            &[rec(now, "local-model", 100), rec(now - 49_800, "local-model", 500)],
        );
        let today = summary_at(&path, 1, now, None).unwrap();
        assert_eq!(today.answers, 1);
        assert_eq!(today.output_tokens, 100);
        // The rolling two-day window still sees both.
        let rolling = summary_at(&path, 2, now + 86_400, None).unwrap();
        assert_eq!(rolling.answers, 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn all_time_skips_no_records() {
        let dir = temp_dir_for("all");
        let path = dir.join("usage.jsonl");
        write_log(&path, &[rec(1, "old", 10), rec(2_000_000_000, "new", 20)]);
        let all = summary_at(&path, 0, 2_000_000_000, None).unwrap();
        assert_eq!(all.answers, 2);
        assert_eq!(all.output_tokens, 30);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn folder_filter_and_projects_rollup() {
        let dir = temp_dir_for("folder");
        let path = dir.join("usage.jsonl");
        let now = 1_900_000_000;
        write_log(
            &path,
            &[
                rec_in(now, 300, Some("C:\\code\\finance")),
                rec_in(now - 60, 200, Some("C:\\code\\finance")),
                rec_in(now - 120, 700, Some("C:\\code\\site")),
                rec_in(now - 180, 50, None),
            ],
        );
        // One project's totals only.
        let finance = summary_at(&path, 7, now, Some("C:\\code\\finance")).unwrap();
        assert_eq!(finance.answers, 2);
        assert_eq!(finance.output_tokens, 500);
        // The project rollup always covers the whole window.
        assert_eq!(finance.per_folder.len(), 2);
        assert_eq!(finance.per_folder[0].folder, "C:\\code\\site");
        assert_eq!(finance.per_folder[0].output, 700);
        assert_eq!(finance.per_folder[1].folder, "C:\\code\\finance");
        assert_eq!(finance.per_folder[1].answers, 2);
        // Plain chats (no folder) count in the totals but not in projects.
        let all = summary_at(&path, 7, now, None).unwrap();
        assert_eq!(all.answers, 4);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sessions_and_streak_count_from_the_log() {
        let dir = temp_dir_for("sessions");
        let path = dir.join("usage.jsonl");
        let now = 1_950_000_000;
        let today = date_from_ts(now);
        let yesterday = date_from_ts(now - 86_400);
        let mut a = rec(now, "local-model", 10);
        a.session = Some("t1".into());
        let mut b = rec(now - 60, "local-model", 10);
        b.session = Some("t1".into());
        let mut c = rec(now - 120, "local-model", 10);
        c.session = Some("t2".into());
        let mut d = rec_in(now - 86_400, 10, None);
        d.session = Some("t2".into());
        let sessionless = rec(now - 30, "local-model", 10);
        write_log(&path, &[a, b, c, d, sessionless]);
        let sum = summary_at(&path, 7, now, None).unwrap();
        // t1 twice, t2 across two days, plus one record with no session id,
        // which counts as its own session.
        assert_eq!(sum.sessions, 3);
        assert_eq!(sum.active_days, 2);
        // Both days active and consecutive: streak 2.
        assert_eq!(sum.longest_streak, 2);
        let _ = (yesterday, today);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn streak_breaks_on_a_gap() {
        let dir = temp_dir_for("gap");
        let path = dir.join("usage.jsonl");
        let now = 1_960_000_000;
        // Active today and three days back, nothing between: streak is 1.
        write_log(&path, &[rec(now, "local-model", 10), rec(now - 3 * 86_400, "local-model", 10)]);
        let sum = summary_at(&path, 7, now, None).unwrap();
        assert_eq!(sum.active_days, 2);
        assert_eq!(sum.longest_streak, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dates_come_out_as_utc_days() {
        // 2027-01-01T00:00:00Z = 1798761600.
        assert_eq!(date_from_ts(1_798_761_600), "2027-01-01");
        // The Unix epoch itself.
        assert_eq!(date_from_ts(0), "1970-01-01");
    }
}
