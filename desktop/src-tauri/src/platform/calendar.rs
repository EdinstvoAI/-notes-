//! Календарь Apple через EventKit — порт `CalendarSync` (TasksModel.swift): доступ, список календарей,
//! события недели, задачи с датой → события в календаре для задач. На Windows системного календаря для
//! обычных программ нет (только приложения из Store) — там команды отвечают «нет», а экран остаётся на задачах.
//!
//! Связи «задача → событие» — `calendar-links.json` в папке настроек, не в хранилище.

use serde::Serialize;

#[derive(Serialize, Default)]
pub struct State {
    /// Есть ли системный календарь на этой ОС.
    pub supported: bool,
    /// "full" — можно читать и писать, "none" — ещё не спрашивали, "denied" — запрещено в настройках.
    pub access: &'static str,
}

#[derive(Serialize)]
pub struct CalendarInfo {
    pub id: String,
    pub title: String,
    pub color: String,
    /// Календарь по умолчанию для новых событий — его не удаляют.
    pub main: bool,
}

#[derive(Serialize)]
pub struct EventInfo {
    pub id: String,
    pub title: String,
    /// мс от 1970.
    pub start: f64,
    pub end: f64,
    pub all_day: bool,
    pub calendar: String,
    pub color: String,
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use block2::RcBlock;
    use notes_core::calplan::{self, Action, Current, Wanted};
    use objc2::runtime::Bool;
    use objc2::rc::Retained;
    use objc2_app_kit::NSColorSpace;
    use objc2_event_kit::{EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStore, EKSpan};
    use objc2_foundation::{NSDate, NSError, NSString};
    use std::collections::BTreeMap;
    use std::path::Path;

    const NOTE: &str = "Задача из заметок <notes>.";

    fn store() -> Retained<EKEventStore> { unsafe { EKEventStore::new() } }
    fn date(ms: f64) -> Retained<NSDate> { NSDate::dateWithTimeIntervalSince1970(ms / 1000.0) }
    fn ms(d: &NSDate) -> f64 { d.timeIntervalSince1970() * 1000.0 }
    fn err(e: Retained<NSError>) -> String { e.localizedDescription().to_string() }

    pub fn state() -> State {
        let s = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
        let access = if s == EKAuthorizationStatus::FullAccess { "full" }
            else if s == EKAuthorizationStatus::NotDetermined { "none" } else { "denied" };
        State { supported: true, access }
    }

    /// Системный вопрос «дать доступ к календарю». Ждёт ответа человека.
    pub fn request() -> bool {
        let (tx, rx) = std::sync::mpsc::channel();
        let block = RcBlock::new(move |granted: Bool, _e: *mut NSError| { let _ = tx.send(granted.as_bool()); });
        unsafe { store().requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&block)) };
        rx.recv().unwrap_or(false)
    }

    fn hex(c: &EKCalendar) -> String {
        unsafe {
            let color = c.color();
            match color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace()) {
                Some(rgb) => format!("#{:02x}{:02x}{:02x}", (rgb.redComponent() * 255.0) as u8, (rgb.greenComponent() * 255.0) as u8, (rgb.blueComponent() * 255.0) as u8),
                None => "#e54d33".into(),
            }
        }
    }

    pub fn calendars() -> Vec<CalendarInfo> {
        let s = store();
        unsafe {
            let main = s.defaultCalendarForNewEvents().map(|c| c.calendarIdentifier().to_string());
            s.calendarsForEntityType(EKEntityType::Event).iter()
                .filter(|c| c.allowsContentModifications())
                .map(|c| { let id = c.calendarIdentifier().to_string(); CalendarInfo { main: main.as_deref() == Some(&id), title: c.title().to_string(), color: hex(&c), id } })
                .collect()
        }
    }

    pub fn events(start: f64, end: f64) -> Vec<EventInfo> {
        if state().access != "full" { return vec![]; }
        let s = store();
        unsafe {
            let p = s.predicateForEventsWithStartDate_endDate_calendars(&date(start), &date(end), None);
            let mut list: Vec<EventInfo> = s.eventsMatchingPredicate(&p).iter().map(|e| {
                let cal = e.calendar();
                EventInfo {
                    id: e.eventIdentifier().map(|i| i.to_string()).unwrap_or_default(),
                    title: e.title().to_string(),
                    start: ms(&e.startDate()), end: ms(&e.endDate()), all_day: e.isAllDay(),
                    calendar: cal.as_ref().map(|c| c.calendarIdentifier().to_string()).unwrap_or_default(),
                    color: cal.as_ref().map(|c| hex(c)).unwrap_or_else(|| "#e54d33".into()),
                }
            }).collect();
            list.sort_by(|a, b| a.start.total_cmp(&b.start));
            list
        }
    }

    fn event(s: &EKEventStore, id: &str) -> Option<Retained<EKEvent>> { unsafe { s.eventWithIdentifier(&NSString::from_str(id)) } }

    pub fn move_event(id: &str, start: f64, end: f64) -> Result<(), String> {
        let s = store();
        let e = event(&s, id).ok_or("события нет")?;
        unsafe { e.setStartDate(Some(&date(start))); e.setEndDate(Some(&date(end))); s.saveEvent_span_commit_error(&e, EKSpan::ThisEvent, true).map_err(err) }
    }

    pub fn rename_event(id: &str, title: &str) -> Result<(), String> {
        let s = store();
        let e = event(&s, id).ok_or("события нет")?;
        unsafe { e.setTitle(Some(&NSString::from_str(title))); s.saveEvent_span_commit_error(&e, EKSpan::ThisEvent, true).map_err(err) }
    }

    pub fn remove_event(id: &str) -> Result<(), String> {
        let s = store();
        let e = event(&s, id).ok_or("события нет")?;
        unsafe { s.removeEvent_span_commit_error(&e, EKSpan::ThisEvent, true).map_err(err) }
    }

    /// Новый календарь — в той же учётной записи, что календарь по умолчанию (iCloud или «На Mac»).
    pub fn create_calendar(title: &str) -> Result<(), String> {
        let s = store();
        unsafe {
            let c = EKCalendar::calendarForEntityType_eventStore(EKEntityType::Event, &s);
            c.setTitle(&NSString::from_str(title));
            let source = s.defaultCalendarForNewEvents().and_then(|d| d.source()).ok_or("нет учётной записи календаря")?;
            c.setSource(Some(&source));
            s.saveCalendar_commit_error(&c, true).map_err(err)
        }
    }

    pub fn rename_calendar(id: &str, title: &str) -> Result<(), String> {
        let s = store();
        unsafe {
            let c = s.calendarWithIdentifier(&NSString::from_str(id)).ok_or("календаря нет")?;
            c.setTitle(&NSString::from_str(title));
            s.saveCalendar_commit_error(&c, true).map_err(err)
        }
    }

    fn links(file: &Path) -> BTreeMap<String, String> {
        std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn linked(file: &Path) -> BTreeMap<String, String> { links(file) }

    /// Задачи с датой → события в календаре для задач (`calendar` — выбранный, иначе по умолчанию).
    pub fn sync(tasks: &[Wanted], calendar: Option<&str>, minutes: u32, file: &Path) -> Result<usize, String> {
        if state().access != "full" { return Ok(0); }
        let s = store();
        let mut links = links(file);
        let n = unsafe {
            let target = calendar.and_then(|id| s.calendarWithIdentifier(&NSString::from_str(id)))
                .or_else(|| s.defaultCalendarForNewEvents()).ok_or("нет календаря для задач")?;
            let mut existing = BTreeMap::new();
            for id in links.values() {
                existing.insert(id.clone(), event(&s, id).map(|e| Current { title: e.title().to_string(), start: ms(&e.startDate()), all_day: e.isAllDay() }));
            }
            let fill = |e: &EKEvent, t: &Wanted| {
                e.setTitle(Some(&NSString::from_str(&t.title)));
                e.setAllDay(t.all_day);
                e.setStartDate(Some(&date(t.start)));
                let end = if t.all_day { t.start } else { t.start + f64::from(t.minutes.unwrap_or(minutes)) * 60_000.0 };
                e.setEndDate(Some(&date(end)));
                e.setNotes(Some(&NSString::from_str(NOTE)));
            };
            let plan = calplan::actions(tasks, &links, &existing);
            let n = plan.len();
            for a in plan {
                match a {
                    Action::Create(t) => {
                        // Такое событие уже сделала Swift-версия (та же метка, название и начало) — берём его, а не дублируем.
                        let p = s.predicateForEventsWithStartDate_endDate_calendars(&date(t.start - 1000.0), &date(t.start + 86_400_000.0), None);
                        let twin = s.eventsMatchingPredicate(&p).iter().find(|e| e.title().to_string() == t.title
                            && (ms(&e.startDate()) - t.start).abs() < 1000.0
                            && e.notes().is_some_and(|n| n.to_string() == NOTE)
                            && !links.values().any(|v| Some(v.as_str()) == e.eventIdentifier().map(|i| i.to_string()).as_deref()));
                        if let Some(id) = twin.and_then(|e| e.eventIdentifier()) { links.insert(t.key, id.to_string()); continue; }
                        let e = EKEvent::eventWithEventStore(&s);
                        e.setCalendar(Some(&target));
                        fill(&e, &t);
                        if s.saveEvent_span_commit_error(&e, EKSpan::ThisEvent, false).is_ok() {
                            if let Some(id) = e.eventIdentifier() { links.insert(t.key, id.to_string()); }
                        }
                    }
                    Action::Update(id, t) => if let Some(e) = event(&s, &id) { fill(&e, &t); let _ = s.saveEvent_span_commit_error(&e, EKSpan::ThisEvent, false); },
                    Action::Delete(key, id) => {
                        if let Some(e) = event(&s, &id) { let _ = s.removeEvent_span_commit_error(&e, EKSpan::ThisEvent, false); }
                        links.remove(&key);
                    }
                }
            }
            s.commit().map_err(err)?;
            n
        };
        if let Some(dir) = file.parent() { let _ = std::fs::create_dir_all(dir); }
        std::fs::write(file, serde_json::to_string(&links).unwrap_or_default()).map_err(|e| e.to_string())?;
        Ok(n)
    }
}

#[cfg(target_os = "macos")]
pub use mac::*;

#[cfg(not(target_os = "macos"))]
mod other {
    use super::*;
    use notes_core::calplan::Wanted;
    use std::collections::BTreeMap;
    use std::path::Path;
    const NO: &str = "системного календаря на этой ОС нет";
    pub fn state() -> State { State { supported: false, access: "denied" } }
    pub fn request() -> bool { false }
    pub fn calendars() -> Vec<CalendarInfo> { vec![] }
    pub fn events(_: f64, _: f64) -> Vec<EventInfo> { vec![] }
    pub fn move_event(_: &str, _: f64, _: f64) -> Result<(), String> { Err(NO.into()) }
    pub fn rename_event(_: &str, _: &str) -> Result<(), String> { Err(NO.into()) }
    pub fn remove_event(_: &str) -> Result<(), String> { Err(NO.into()) }
    pub fn create_calendar(_: &str) -> Result<(), String> { Err(NO.into()) }
    pub fn rename_calendar(_: &str, _: &str) -> Result<(), String> { Err(NO.into()) }
    pub fn linked(_: &Path) -> BTreeMap<String, String> { BTreeMap::new() }
    pub fn sync(_: &[Wanted], _: Option<&str>, _: u32, _: &Path) -> Result<usize, String> { Ok(0) }
}

#[cfg(not(target_os = "macos"))]
pub use other::*;
