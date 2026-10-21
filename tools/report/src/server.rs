//! The routes: each fetches what it shows from the store and hands it to
//! its page.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use chronicle::{RUNS_KEPT, Run};
use serde::Deserialize;

use crate::fold::Report;
use crate::pages;
use crate::store::Store;
use crate::summary::summary;

pub fn router(store: Store) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/runs/{id}", get(run))
        .route("/runs/{id}/summary.txt", get(summary_text))
        .route("/compare", get(compare))
        .route("/trend", get(trend))
        .route("/crashes", get(crashes))
        .route("/crashes/{id}", get(crash))
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
    let text = summary(id, &report, previous.as_deref().zip(before.as_ref()));
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

    fn sample() -> Store {
        let store = store(
            "routes",
            &[("2023-01-01-soak", 3.0), ("2023-01-02-soak", 4.0)],
        );
        std::fs::write(store.crashes.join("2023-01-02-soak.txt"), "panicked at sea").unwrap();
        store
    }

    #[tokio::test]
    async fn every_page_shows_what_it_should() {
        let store = sample();
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

    #[tokio::test]
    async fn an_unknown_or_crafted_id_finds_nothing() {
        let store = sample();
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
