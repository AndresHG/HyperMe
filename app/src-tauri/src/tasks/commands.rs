//! The commands the Today page calls.

use super::*;

pub(super) async fn page_now(app: &tauri::AppHandle) -> Result<TodayPage, String> {
    use hyperme_platform::presence::MacClock;
    let (store, rules) = state(app)?;
    let now = UtcDateTime::now();
    let view = load_today(&store, &MacClock, &rules, now).await?;
    let meetings = sync::meetings_on(&*store, view.date, &MacClock)?;
    Ok(today_page(&view, &meetings, now, &MacClock))
}

/// The Today block for Home.
#[tauri::command]
#[specta::specta]
pub async fn home_today(app: tauri::AppHandle) -> Result<TodayPage, String> {
    page_now(&app).await
}

/// "Add a task": a title, for today.
#[tauri::command]
#[specta::specta]
pub async fn tasks_add(app: tauri::AppHandle, title: String) -> Result<(), String> {
    let (store, rules) = state(&app)?;
    let now = UtcDateTime::now();
    let (today, _) = today_of(&hyperme_platform::presence::MacClock, &rules, now);
    add_today(&store, &title, today, ms(now)).await?;
    plan_nudges(&app);
    Ok(())
}

/// A row action: `done`, `drop`, `tomorrow`, `keep`, `move` with `value` a `YYYY-MM-DD` day,
/// or `rename` with `value` the new title.
#[tauri::command]
#[specta::specta]
pub async fn tasks_act(
    app: tauri::AppHandle,
    id: TaskId,
    action: String,
    value: Option<String>,
) -> Result<(), String> {
    let (store, rules) = state(&app)?;
    let now = UtcDateTime::now();
    let (today, _) = today_of(&hyperme_platform::presence::MacClock, &rules, now);
    let action = match (action.as_str(), value) {
        ("done", _) => Act::Done,
        ("drop", _) => Act::Drop,
        ("tomorrow", _) => Act::Tomorrow,
        ("keep", _) => Act::Keep,
        ("rename", Some(title)) => Act::Rename(title),
        ("move", Some(day)) => Act::Move(parse_day(&day).ok_or(format!("{day} is not a day"))?),
        (other, _) => return Err(format!("{other} is not a task action")),
    };
    act(&store, id, action, today, ms(now)).await?;
    plan_nudges(&app);
    Ok(())
}

/// Opens a meeting's link in the browser; only web links.
#[tauri::command]
#[specta::specta]
pub fn tasks_open_meeting(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Only web links open from Home".to_owned());
    }
    std::process::Command::new("open")
        .arg(url)
        .status()
        .map(drop)
        .map_err(|e| e.to_string())
}
