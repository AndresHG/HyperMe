//! An in-memory [`TaskStore`] for tests of the rules and, later, of the card builders and the
//! scheduler. It applies the same pure rules as the SQLite store will (slice 2).

use std::sync::Mutex;

use hyperme_core::tasks::{
    Finish, NewTask, Source, Status, Task, TaskError, TaskId, TaskStore, TaskUpdate, cycle,
    roll_over,
};
use time::Date;

#[derive(Debug, Default)]
pub struct MemoryTaskStore {
    tasks: Mutex<Vec<Task>>,
}

impl MemoryTaskStore {
    fn with<R>(&self, f: impl FnOnce(&mut Vec<Task>) -> R) -> R {
        f(&mut self.tasks.lock().unwrap())
    }
}

fn find(tasks: &mut [Task], id: TaskId) -> Result<&mut Task, TaskError> {
    tasks
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or(TaskError::NotFound(id))
}

/// What identifies a mirrored item: source name, account and reference.
fn mirror_key(source: &Source) -> Option<(&str, Option<&str>, &str)> {
    match source {
        Source::HyperMe => None,
        Source::Mirror {
            name,
            account,
            reference,
            ..
        } => Some((name, account.as_deref(), reference)),
    }
}

impl TaskStore for MemoryTaskStore {
    async fn add(&self, new: NewTask) -> Result<TaskId, TaskError> {
        self.with(|tasks| {
            if let Some(&missing) = new
                .blocked_by
                .iter()
                .find(|b| !tasks.iter().any(|t| t.id == **b))
            {
                return Err(TaskError::NotFound(missing));
            }
            let id = tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            let mut task = Task::new(id, &new.title, new.created_at, new.arrived_day);
            task.notes = new.notes;
            task.day = new.day;
            task.due = new.due;
            task.source = new.source;
            task.blocked_by = new.blocked_by;
            tasks.push(task);
            Ok(id)
        })
    }

    async fn update(&self, id: TaskId, change: TaskUpdate) -> Result<(), TaskError> {
        self.with(|tasks| {
            let task = find(tasks, id)?;
            if let Some(title) = change.title {
                task.title = title;
            }
            if let Some(notes) = change.notes {
                task.notes = notes;
            }
            if let Some(day) = change.day {
                task.day = day;
            }
            if let Some(due) = change.due {
                task.due = due;
            }
            Ok(())
        })
    }

    async fn finish(&self, id: TaskId, how: Finish, at_ms: i64) -> Result<(), TaskError> {
        self.with(|tasks| {
            let task = find(tasks, id)?;
            task.status = match how {
                Finish::Done => Status::Done,
                Finish::Dropped => Status::Dropped,
            };
            task.finished_at = Some(at_ms);
            Ok(())
        })
    }

    async fn remove(&self, id: TaskId) -> Result<(), TaskError> {
        self.with(|tasks| {
            find(tasks, id)?;
            tasks.retain(|t| t.id != id);
            for t in tasks.iter_mut() {
                t.blocked_by.retain(|b| *b != id);
            }
            Ok(())
        })
    }

    async fn upsert_mirror(
        &self,
        new: NewTask,
        closed_at: Option<i64>,
    ) -> Result<TaskId, TaskError> {
        let existing = self.with(|tasks| {
            tasks
                .iter()
                .find(|t| mirror_key(&new.source).is_some_and(|k| mirror_key(&t.source) == Some(k)))
                .map(|t| t.id)
        });
        let id = match existing {
            Some(id) => {
                self.with(|tasks| {
                    let task = find(tasks, id)?;
                    task.title = new.title;
                    task.notes = new.notes;
                    task.due = new.due;
                    task.source = new.source;
                    Ok::<_, TaskError>(())
                })?;
                id
            }
            None => self.add(new).await?,
        };
        if let Some(at) = closed_at {
            self.with(|tasks| {
                let task = find(tasks, id)?;
                if task.status == Status::Open {
                    task.status = Status::Done;
                    task.finished_at = Some(at);
                }
                Ok::<_, TaskError>(())
            })?;
        }
        Ok(id)
    }

    async fn block(&self, blocked: TaskId, blocker: TaskId) -> Result<(), TaskError> {
        self.with(|tasks| {
            find(tasks, blocker)?;
            find(tasks, blocked)?;
            if let Some(path) = cycle(tasks, blocked, blocker) {
                return Err(TaskError::Cycle(path));
            }
            let task = find(tasks, blocked)?;
            if !task.blocked_by.contains(&blocker) {
                task.blocked_by.push(blocker);
            }
            Ok(())
        })
    }

    async fn unblock(&self, blocked: TaskId, blocker: TaskId) -> Result<(), TaskError> {
        self.with(|tasks| {
            find(tasks, blocked)?.blocked_by.retain(|b| *b != blocker);
            Ok(())
        })
    }

    async fn roll_over(&self, today: Date) -> Result<Vec<TaskId>, TaskError> {
        self.with(|tasks| Ok(roll_over(tasks, today)))
    }

    async fn snapshot(&self, today: Date, day_began_ms: i64) -> Result<Vec<Task>, TaskError> {
        // Small enough to hand back everything; the rules pick what belongs to Today.
        let _ = (today, day_began_ms);
        self.with(|tasks| Ok(tasks.clone()))
    }
}
