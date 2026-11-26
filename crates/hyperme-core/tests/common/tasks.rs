//! An in-memory [`TaskStore`] for tests of the rules and, later, of the card builders and the
//! scheduler. It applies the same pure rules as the SQLite store will (slice 2).

use std::sync::Mutex;

use hyperme_core::tasks::{
    Finish, NewTask, Status, Task, TaskError, TaskId, TaskStore, TaskUpdate, cycle, roll_over,
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
