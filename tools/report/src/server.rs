//! The routes: each fetches what it shows from the store and hands it to
//! its page.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use chronicle::{RUNS_KEPT, Run};
use maud::Markup;
use serde::Deserialize;
use tuning::{KNOBS_FILE, Knob, TARGETS_FILE, Target};

use crate::balance::verdicts;
use crate::fold::Report;
use crate::pages;
use crate::store::Store;
use crate::summary::summary;
use crate::tuning_page::{Tuning, tuning_page};

pub fn router(store: Store) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/runs/{id}", get(run))
        .route("/runs/{id}/summary.txt", get(summary_text))
        .route("/compare", get(compare))
        .route("/trend", get(trend))
        .route("/crashes", get(crashes))
        .route("/crashes/{id}", get(crash))
        .route("/tuning", get(tuning))
        .route("/tuning/knob", post(save_knob))
        .route("/tuning/target", post(save_target))
        .with_state(Arc::new(store))
}

type Shared = State<Arc<Store>>;

fn not_found(what: &str) -> Response {
    (StatusCode::NOT_FOUND, pages::not_found(what)).into_response()
}

/// A run's report and its summary, against the run of its kind before it
/// if there is one, and that run's id.
pub fn summarised(store: &Store, id: &str) -> Result<(Report, String, Option<String>), String> {
    let report = store.report(id)?;
    let previous = store.previous(id, &report);
    let before = previous
        .as_deref()
        .and_then(|previous| store.report(previous).ok());
    let targets: Vec<Target> = store.tuning(TARGETS_FILE).unwrap_or_default();
    let text = summary(
        id,
        &report,
        previous.as_deref().zip(before.as_ref()),
        &verdicts(&targets, &report),
    );
    Ok((report, text, previous))
}

#[derive(Deserialize)]
struct Kind {
    kind: Option<String>,
}

fn run_kind(name: &str) -> Option<Run> {
    [Run::Play, Run::Soak]
        .into_iter()
        .find(|run| run.name() == name)
}

/// Every run with a report, newest first.
fn reports(store: &Store) -> Vec<(String, Report)> {
    store
        .runs()
        .into_iter()
        .filter_map(|id| Some((id.clone(), store.report(&id).ok()?)))
        .collect()
}

async fn index(State(store): Shared, Query(kind): Query<Kind>) -> Response {
    let kind = kind.kind.as_deref().and_then(run_kind);
    pages::index(&reports(&store), kind).into_response()
}

async fn run(State(store): Shared, Path(id): Path<String>) -> Response {
    match summarised(&store, &id) {
        Ok((report, text, previous)) => pages::run_page(
            &id,
            &report,
            &text,
            previous.as_deref(),
            store.crash(&id).is_some(),
        )
        .into_response(),
        Err(error) => not_found(&error),
    }
}

async fn summary_text(State(store): Shared, Path(id): Path<String>) -> Response {
    match summarised(&store, &id) {
        Ok((_, text, _)) => {
            ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response()
        }
        Err(error) => not_found(&error),
    }
}

#[derive(Deserialize)]
struct Pair {
    a: Option<String>,
    b: Option<String>,
}

async fn compare(State(store): Shared, Query(pair): Query<Pair>) -> Response {
    let (Some(a), Some(b)) = (pair.a, pair.b) else {
        return not_found("Pick a run in each of A and B to compare.");
    };
    match (store.report(&a), store.report(&b)) {
        (Ok(before), Ok(after)) => pages::compare_page((&a, &before), (&b, &after)).into_response(),
        (Err(error), _) | (_, Err(error)) => not_found(&error),
    }
}

async fn trend(State(store): Shared, Query(kind): Query<Kind>) -> Response {
    let kind = kind.kind.as_deref().and_then(run_kind).unwrap_or(Run::Soak);
    let mut runs: Vec<(String, Report)> = reports(&store)
        .into_iter()
        .filter(|(_, report)| report.run == kind)
        .take(RUNS_KEPT)
        .collect();
    runs.reverse();
    pages::trend_page(kind, &runs).into_response()
}

async fn crashes(State(store): Shared) -> Response {
    pages::crashes_page(&store.crashes()).into_response()
}

async fn crash(State(store): Shared, Path(id): Path<String>) -> Response {
    match store.crash(&id) {
        Some(text) => pages::crash_page(&id, &text).into_response(),
        None => not_found(&format!("There's no crash report {id}.")),
    }
}

#[derive(Deserialize)]
struct Saved {
    saved: Option<String>,
}

async fn tuning(State(store): Shared, Query(saved): Query<Saved>) -> Response {
    let notice = saved.saved.map(|id| Ok(format!("Saved {id}.")));
    tuning_view(&store, notice).into_response()
}

/// The Tuning page, with the targets measured on the newest soak.
fn tuning_view(store: &Store, notice: Option<Result<String, String>>) -> Markup {
    let targets = store.tuning::<Target>(TARGETS_FILE);
    let newest_soak = store.runs().into_iter().find_map(|id| {
        let report = store.report(&id).ok()?;
        (report.run == Run::Soak).then_some((id, report))
    });
    let measured = match (&targets, &newest_soak) {
        (Ok(targets), Some((id, report))) => Some((id.clone(), verdicts(targets, report))),
        _ => None,
    };
    tuning_page(&Tuning {
        knobs: store.tuning(KNOBS_FILE),
        targets: &targets,
        measured,
        notice,
    })
}

/// Back to the entry just saved, or the page again saying why it wasn't.
fn after_saving(store: &Store, id: &str, saved: Result<(), String>) -> Response {
    match saved {
        Ok(()) => Redirect::to(&format!("/tuning?saved={id}#{id}")).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            tuning_view(store, Some(Err(error))),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct KnobForm {
    id: String,
    value: f64,
}

async fn save_knob(State(store): Shared, Form(form): Form<KnobForm>) -> Response {
    let saved = store.tuning::<Knob>(KNOBS_FILE).and_then(|mut knobs| {
        let knob = knobs
            .iter_mut()
            .find(|knob| knob.id == form.id)
            .ok_or_else(|| format!("There's no knob {}.", form.id))?;
        knob.value = form.value;
        store.save_tuning(KNOBS_FILE, &knobs)
    });
    after_saving(&store, &form.id, saved)
}

#[derive(Deserialize)]
struct TargetForm {
    id: String,
    /// Empty for no least or most.
    min: String,
    max: String,
}

/// A form's number, or `None` if it was left empty.
fn bound(text: &str) -> Result<Option<f64>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    text.parse()
        .map(Some)
        .map_err(|_| format!("{text} isn't a number."))
}

async fn save_target(State(store): Shared, Form(form): Form<TargetForm>) -> Response {
    let saved = store
        .tuning::<Target>(TARGETS_FILE)
        .and_then(|mut targets| {
            let target = targets
                .iter_mut()
                .find(|target| target.id == form.id)
                .ok_or_else(|| format!("There's no target {}.", form.id))?;
            target.min = bound(&form.min)?;
            target.max = bound(&form.max)?;
            store.save_tuning(TARGETS_FILE, &targets)
        });
    after_saving(&store, &form.id, saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::store;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    async fn get(store: &Store, uri: &str) -> (StatusCode, String) {
        let response = router(store.clone())
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    /// A store of two soaks, the second of which crashed, in a folder
    /// named for the test: tests run at once, and mustn't share one.
    fn sample(name: &str) -> Store {
        let store = store(name, &[("2023-01-01-soak", 3.0), ("2023-01-02-soak", 4.0)]);
        std::fs::write(store.crashes.join("2023-01-02-soak.txt"), "panicked at sea").unwrap();
        store
    }

    #[tokio::test]
    async fn every_page_shows_what_it_should() {
        let store = sample("routes");
        let (status, index) = get(&store, "/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(index.contains("2023-01-01-soak") && index.contains("2023-01-02-soak"));

        let (_, run) = get(&store, "/runs/2023-01-02-soak").await;
        assert!(run.contains("Copy summary for Claude"));
        assert!(run.contains("/compare?a=2023-01-01-soak&amp;b=2023-01-02-soak"));
        assert!(run.contains("/crashes/2023-01-02-soak"));

        let (_, text) = get(&store, "/runs/2023-01-02-soak/summary.txt").await;
        assert!(text.starts_with("Run 2023-01-02-soak:"));
        assert!(text.contains("Changed since run 2023-01-01-soak"));

        let (_, compared) = get(&store, "/compare?a=2023-01-01-soak&b=2023-01-02-soak").await;
        assert!(compared.contains("A against B"));
        let (_, trend) = get(&store, "/trend?kind=soak").await;
        assert!(trend.contains("The trend over soak runs"));
        let (_, crash) = get(&store, "/crashes/2023-01-02-soak").await;
        assert!(crash.contains("panicked at sea"));
    }

    async fn post(store: &Store, uri: &str, form: &str) -> StatusCode {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form.to_string()))
            .unwrap();
        router(store.clone())
            .oneshot(request)
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn the_tuning_page_shows_each_knob_with_its_hints_and_saves_it_in_range() {
        let store = sample("tuning");
        std::fs::create_dir_all(&store.data).unwrap();
        for file in [KNOBS_FILE, TARGETS_FILE] {
            let game = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
            std::fs::copy(game.join(file), store.data.join(file)).unwrap();
        }
        let (status, page) = get(&store, "/tuning").await;
        assert_eq!(status, StatusCode::OK);
        assert!(page.contains("What a whole company pumps") && page.contains("Chain pumps"));
        assert!(page.contains("Ships foundered"));
        assert!(page.contains("Measured on soak <a href=\"/runs/2023-01-02-soak\">"));

        let pumps = |store: &Store| {
            store
                .tuning::<Knob>(KNOBS_FILE)
                .unwrap()
                .into_iter()
                .find(|knob| knob.id == "leaks.pumps_per_hour")
                .unwrap()
                .value
        };
        let status = post(&store, "/tuning/knob", "id=leaks.pumps_per_hour&value=0.12").await;
        assert_eq!(status, StatusCode::SEE_OTHER);
        assert_eq!(pumps(&store), 0.12);
        let status = post(&store, "/tuning/knob", "id=leaks.pumps_per_hour&value=99").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "out of its range");
        assert_eq!(pumps(&store), 0.12, "and left as it was");

        let status = post(
            &store,
            "/tuning/target",
            "id=foundered_per_100_days&min=&max=3",
        )
        .await;
        assert_eq!(status, StatusCode::SEE_OTHER);
        let targets = store.tuning::<Target>(TARGETS_FILE).unwrap();
        let foundered = targets
            .iter()
            .find(|target| target.id == "foundered_per_100_days")
            .unwrap();
        assert_eq!((foundered.min, foundered.max), (None, Some(3.0)));
    }

    #[tokio::test]
    async fn an_unknown_or_crafted_id_finds_nothing() {
        let store = sample("crafted");
        for uri in [
            "/runs/2023-09-09-soak",
            "/runs/..%2F..%2Fsecret",
            "/crashes/..%2Flogs%2F2023-01-01-soak",
            "/compare?a=2023-01-01-soak",
        ] {
            assert_eq!(get(&store, uri).await.0, StatusCode::NOT_FOUND, "{uri}");
        }
    }
}
