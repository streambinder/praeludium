//! HTTP-level integration tests: the whole route table against
//! an in-memory database, one fresh app per test.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use praeludium::db::{App, open_memory};
use praeludium::handlers::router;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

fn test_app() -> (axum::Router, Arc<App>) {
    let app = Arc::new(App {
        db: Mutex::new(open_memory().unwrap()),
        data_dir: std::env::temp_dir().join(format!("praeludium-test-{}", std::process::id())),
        current_user_id: praeludium::db::SEEDED_USER_ID,
    });
    let r = router(app.clone());
    (r, app)
}

fn test_router() -> axum::Router {
    test_app().0
}

/// Insert an audition directly and enroll the user in it.
/// Returns the auto-assigned id.
fn seed_audition(app: &App, name: &str, date: &str, weight: i64) -> i64 {
    let db = app.db.lock().unwrap();
    db.execute(
        "INSERT INTO auditions(name, audition_date, weight) VALUES(?,?,?)",
        rusqlite::params![name, date, weight],
    )
    .unwrap();
    let id = db.last_insert_rowid();
    praeludium::db::ensure_test_user(&db).unwrap();
    db.execute(
        "INSERT INTO enrollments(user_id, audition_id) VALUES(1, ?)",
        rusqlite::params![id],
    )
    .unwrap();
    id
}

/// Insert an audition directly without enrolling.
fn seed_open_audition(app: &App, name: &str, date: &str, weight: i64) -> i64 {
    let db = app.db.lock().unwrap();
    db.execute(
        "INSERT INTO auditions(name, audition_date, weight) VALUES(?,?,?)",
        rusqlite::params![name, date, weight],
    )
    .unwrap();
    db.last_insert_rowid()
}

/// Insert a piece directly. Returns the auto-assigned id.
fn seed_piece(app: &App, composer: &str, work: &str, kind: &str, difficulty: i64) -> i64 {
    let db = app.db.lock().unwrap();
    db.execute(
        "INSERT INTO pieces(composer, work, kind, difficulty) VALUES(?,?,?,?)",
        rusqlite::params![composer, work, kind, difficulty],
    )
    .unwrap();
    db.last_insert_rowid()
}

/// Link a piece to an audition directly.
fn link(app: &App, piece: i64, audition: i64) {
    let db = app.db.lock().unwrap();
    db.execute(
        "INSERT INTO piece_audition(piece_id, audition_id) VALUES(?,?)",
        rusqlite::params![piece, audition],
    )
    .unwrap();
}

/// Link a piece to an audition with a note and an optional flag.
fn link_full(app: &App, piece: i64, audition: i64, note: &str, optional: bool) {
    let db = app.db.lock().unwrap();
    db.execute(
        "INSERT INTO piece_audition(piece_id, audition_id, note, optional) VALUES(?,?,?,?)",
        rusqlite::params![piece, audition, note, optional as i64],
    )
    .unwrap();
}

/// Insert a round directly. Returns the auto-assigned id.
fn seed_round(app: &App, audition: i64, name: &str) -> i64 {
    let db = app.db.lock().unwrap();
    db.execute(
        "INSERT INTO audition_rounds(audition_id, position, name) VALUES(?, 1, ?)",
        rusqlite::params![audition, name],
    )
    .unwrap();
    db.last_insert_rowid()
}

/// Declare a baseline for every piece that lacks a valued session.
/// The home plan renders only once no active piece waits for a
/// baseline, so plan tests declare baselines for the seeded
/// repertoire too.
fn declare_all_baselines(app: &App) {
    let db = app.db.lock().unwrap();
    db.execute(
        "INSERT INTO sessions(date, piece_id, minutes, confidence, note)
         SELECT '2026-10-20', id, 0, 3, 'baseline' FROM pieces
         WHERE id NOT IN (SELECT piece_id FROM sessions)",
        [],
    )
    .unwrap();
}

/// Insert one session row directly, for diary and plan fixtures.
fn seed_session(app: &App, date: &str, piece: i64, minutes: i64, conf: i64, note: &str) {
    let db = app.db.lock().unwrap();
    db.execute(
        "INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES(?,?,?,?,?)",
        rusqlite::params![date, piece, minutes, conf, note],
    )
    .unwrap();
}

async fn body_string(resp: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 22)
        .await
        .unwrap();
    String::from_utf8_lossy(&bytes).to_string()
}

fn req(method: &str, path: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .unwrap()
}

fn form_req(path: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[tokio::test]
async fn empty_app_pages_render() {
    let r = test_router();
    for path in ["/", "/pieces", "/auditions", "/diary", "/settings"] {
        let resp = r.clone().oneshot(req("GET", path)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "GET {path}");
        let body = body_string(resp).await;
        assert!(body.contains("præludium"), "GET {path} body");
    }
    // The pages render the seeded repertoire, not empty lists.
    let resp = r.clone().oneshot(req("GET", "/auditions")).await.unwrap();
    assert!(
        body_string(resp).await.contains("Santa Cecilia"),
        "seeded audition lists"
    );
    let resp = r.clone().oneshot(req("GET", "/pieces")).await.unwrap();
    assert!(
        body_string(resp).await.contains("Beethoven"),
        "seeded piece lists"
    );
}

#[tokio::test]
async fn piece_and_session_flow() {
    let (r, app) = test_app();
    let aid = seed_audition(&app, "Test Audition", "2027-01-10", 4);
    let pid = seed_piece(&app, "Bach", "Suite", "solo", 3);
    link(&app, pid, aid);
    let resp = r.clone().oneshot(req("GET", "/pieces")).await.unwrap();
    assert!(body_string(resp).await.contains("Bach"));
    // The piece waits for its baseline declaration on the home page.
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    assert!(body_string(resp).await.contains("baseline"));
    let resp = r
        .clone()
        .oneshot(form_req(&format!("/pieces/{pid}/baseline"), "confidence=3"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/availability",
            &format!("date={}&start=09:00&end=12:00&label=", today()),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={pid}&minutes=30&confidence=4"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r.clone().oneshot(req("GET", "/diary")).await.unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("Bach"), "diary shows the practiced piece");
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/day/{}", today())))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/piece/{pid}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn validation_errors_match() {
    let r = test_router();
    let resp = r
        .clone()
        .oneshot(form_req("/session", "piece_id=0&minutes=1&confidence=3"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(form_req("/settings/language", "lang=xx"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/settings",
            "urgency_k=-1&urgency_horizon=60&recency_cap=1&diff_horizon=0.25&diff_boost=0.075",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary/piece/999"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary/day/2026-13-40"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn language_cookie_flow() {
    let r = test_router();
    let resp = r
        .clone()
        .oneshot(form_req("/settings/language", "lang=it"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let cookie = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.contains("lang=it"), "cookie: {cookie}");
    assert!(cookie.contains("HttpOnly") && cookie.contains("Secure"));
    let resp = r
        .clone()
        .oneshot(form_req("/settings/language", "lang=auto"))
        .await
        .unwrap();
    let cookie = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.contains("Max-Age=0"), "cookie: {cookie}");
}

#[tokio::test]
async fn mock_flow() {
    let (r, app) = test_app();
    let aid = seed_audition(&app, "Mock Cup", "2027-02-01", 3);
    let pid = seed_piece(&app, "Brahms", "Sonata", "excerpt", 5);
    link(&app, pid, aid);
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}/mock")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(form_req(
            &format!("/auditions/{aid}/mock"),
            "count=1&record=1",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).unwrap(),
        &format!("/auditions/{aid}/mock/1")
    );
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}/mock/1")))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("Brahms"), "mock run shows the piece");
    let resp = r
        .clone()
        .oneshot(form_req(
            &format!("/auditions/{aid}/mock/1/rate"),
            "item_id=1&confidence=5&note=ok",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}/mock/1")))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("5/5") || body.contains("5,5") || body.contains('5'));
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}/readiness")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(form_req(&format!("/auditions/{aid}/mock/1/delete"), ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn legacy_redirects_and_static() {
    let r = test_router();
    let resp = r.clone().oneshot(req("GET", "/pezzi")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/pieces");
    let resp = r
        .clone()
        .oneshot(form_req("/sessione", "piece_id=1&minutes=1&confidence=1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/session");
    let resp = r
        .clone()
        .oneshot(req("GET", "/static/style.css"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(req("GET", "/static/nope.css"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = r
        .clone()
        .oneshot(req("GET", "/no-such-page"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(body_string(resp).await, "404 page not found\n");
    let resp = r.clone().oneshot(req("DELETE", "/pieces")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn recording_upload_share_and_range() {
    let dir = std::env::temp_dir().join(format!("praeludium-test-rec-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let app = Arc::new(App {
        db: Mutex::new(open_memory().unwrap()),
        data_dir: dir.clone(),
        current_user_id: praeludium::db::SEEDED_USER_ID,
    });
    let pid = seed_piece(&app, "Bach", "Suite", "solo", 3);
    let r = router(app);
    let boundary = "testboundary42";
    let payload = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"taken_on\"\r\n\r\n{}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"duration_sec\"\r\n\r\n12\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"take.webm\"\r\nContent-Type: audio/webm\r\n\r\nWEBMDATA0123456789\r\n--{boundary}--\r\n",
        today()
    );
    let upload = Request::builder()
        .method("POST")
        .uri(format!("/pieces/{pid}/recordings"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(payload))
        .unwrap();
    let resp = r.clone().oneshot(upload).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("\"id\":1"));
    let resp = r
        .clone()
        .oneshot(req("GET", "/recordings/1/audio"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_string(resp).await, "WEBMDATA0123456789");
    let ranged = Request::builder()
        .method("GET")
        .uri("/recordings/1/audio")
        .header(header::RANGE, "bytes=4-7")
        .body(Body::empty())
        .unwrap();
    let resp = r.clone().oneshot(ranged).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(body_string(resp).await, "DATA");
    let resp = r
        .clone()
        .oneshot(form_req("/recordings/1/share", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/piece/{pid}")))
        .await
        .unwrap();
    let page = body_string(resp).await;
    let start = page.find("/share/").unwrap() + "/share/".len();
    let token: String = page[start..].chars().take(48).collect();
    assert_eq!(token.len(), 48);
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/share/{token}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(form_req("/recordings/1/share/revoke", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/share/{token}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn audition_detail_shows_seed_links() {
    let r = test_router();
    // The seeded Santa Cecilia audition carries links and rounds.
    let resp = r.clone().oneshot(req("GET", "/auditions/1")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(
        body.contains("santacecilia.it"),
        "detail lists the seeded link host"
    );
    assert!(
        body.contains("Bando ufficiale"),
        "detail lists the seeded link label"
    );
    let resp = r
        .clone()
        .oneshot(req("GET", "/auditions/999"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = r
        .clone()
        .oneshot(req("GET", "/auditions/abc"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    // The removed write routes are gone: POST on the kept path
    // answers 405, a removed sub-route answers 404.
    let resp = r
        .clone()
        .oneshot(form_req("/auditions", "name=Cup&date=2027-01-10&weight=4"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/auditions/1/links",
            "link_url=https://example.org/x",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn piece_filters_and_archived_listing() {
    let (r, app) = test_app();
    let aid = seed_audition(&app, "Cup", "2027-01-10", 4);
    let rid = seed_round(&app, aid, "Finale");
    let pid = seed_piece(&app, "Bach", "Suite II", "excerpt", 4);
    {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO piece_audition(piece_id, audition_id, round_id, note) VALUES(?,?,?,?)",
            rusqlite::params![pid, aid, rid, "da capo"],
        )
        .unwrap();
    }
    // Filters scope to the linked excerpt.
    let resp = r
        .clone()
        .oneshot(req(
            "GET",
            &format!("/pieces?audition={aid}&kind=excerpt&q=Suite"),
        ))
        .await
        .unwrap();
    assert!(body_string(resp).await.contains("Suite II"));
    let resp = r
        .clone()
        .oneshot(req("GET", "/pieces?q=nomatch"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // Archive state is seed data: archiving the piece in the
    // database moves it to the archived listing.
    {
        let db = app.db.lock().unwrap();
        db.execute(
            "UPDATE pieces SET archived_at='2026-01-01T00:00:00Z' WHERE id=?",
            rusqlite::params![pid],
        )
        .unwrap();
    }
    let resp = r
        .clone()
        .oneshot(req("GET", "/pieces?archiviati=1"))
        .await
        .unwrap();
    assert!(body_string(resp).await.contains("Suite II"));
    let resp = r
        .clone()
        .oneshot(req("GET", "/pieces/xyz/edit"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn today_skip_and_availability_delete() {
    let (r, app) = test_app();
    let aid = seed_open_audition(&app, "Cup", "2027-01-10", 4);
    // A new audition starts unenrolled: enroll it so the piece
    // reaches the plan.
    r.clone()
        .oneshot(form_req(&format!("/auditions/{aid}/enrollment"), ""))
        .await
        .unwrap();
    let pid = seed_piece(&app, "Bach", "Suite", "solo", 3);
    link(&app, pid, aid);
    r.clone()
        .oneshot(form_req(&format!("/pieces/{pid}/baseline"), "confidence=4"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/availability",
            &format!("date={}&start=08:00&end=23:00&label=", today()),
        ))
        .await
        .unwrap();
    declare_all_baselines(&app);
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("plan-card"), "plan shows the piece: {body}");
    let resp = r
        .clone()
        .oneshot(form_req("/session/skip", &format!("piece_id={pid}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("skipped-card"), "home shows the skip card");
    // Logging minutes after the skip clears the marker.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={pid}&minutes=10&confidence=3"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    assert!(!body_string(resp).await.contains("skipped-card"));
    // The diary day page groups and labels the day.
    seed_session(&app, &today(), pid, 0, 0, "postponed");
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/day/{}", today())))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(form_req("/availability/delete", "id=1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={pid}&minutes=5&confidence=3&bpm=999"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn settings_save_and_diary_filter() {
    let (r, app) = test_app();
    let resp = r
        .clone()
        .oneshot(form_req(
            "/settings",
            "urgency_k=2.5&urgency_horizon=45&recency_cap=1.2&diff_horizon=0.3&diff_boost=0.1",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r.clone().oneshot(req("GET", "/settings")).await.unwrap();
    assert!(body_string(resp).await.contains("2.5"));
    let aid = seed_audition(&app, "Cup", "2027-01-10", 4);
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary?c={aid}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r.clone().oneshot(req("GET", "/diary?c=abc")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(req("GET", "/share/sometoken/audio"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = r
        .clone()
        .oneshot(form_req("/pieces/77/recordings", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mock_with_round_and_rerate_guard() {
    let (r, app) = test_app();
    let aid = seed_audition(&app, "Cup", "2027-01-10", 4);
    let rid = seed_round(&app, aid, "Finale");
    let pid = seed_piece(&app, "Bach", "Suite", "solo", 3);
    {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO piece_audition(piece_id, audition_id, round_id) VALUES(?,?,?)",
            rusqlite::params![pid, aid, rid],
        )
        .unwrap();
    }
    let resp = r
        .clone()
        .oneshot(form_req(&format!("/auditions/{aid}/mock"), "round_id=99"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(form_req(
            &format!("/auditions/{aid}/mock"),
            &format!("round_id={rid}&count=5"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req(
            &format!("/auditions/{aid}/mock/1/rate"),
            "item_id=1&confidence=2",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    // Second rating of the same item is a no-op by design.
    let resp = r
        .clone()
        .oneshot(form_req(
            &format!("/auditions/{aid}/mock/1/rate"),
            "item_id=1&confidence=5",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}")))
        .await
        .unwrap();
    assert!(body_string(resp).await.contains("2"));
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/9/mock", "count=1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn auditions_list_and_diary_markers() {
    let (r, app) = test_app();
    // One concluded audition (past, not archived) and one upcoming
    // with a linked piece and a link row.
    seed_open_audition(&app, "Past Cup", "2020-01-10", 2);
    let next = seed_open_audition(&app, "Next Cup", "2027-01-10", 4);
    {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO audition_links(audition_id, label, url) VALUES(?, 'Bando', 'https://example.org/x')",
            rusqlite::params![next],
        )
        .unwrap();
    }
    let bach = seed_piece(&app, "Bach", "Suite", "solo", 3);
    link(&app, bach, next);
    let resp = r.clone().oneshot(req("GET", "/auditions")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("Past Cup") && body.contains("Next Cup"));
    // Diary day with every marker kind on a fixed past date.
    let day = "2026-10-12";
    seed_session(&app, day, bach, 0, 0, "skipped");
    let vivaldi = seed_piece(&app, "Vivaldi", "Concerto", "solo", 2);
    seed_session(&app, day, vivaldi, 0, 0, "auto-skipped");
    let haydn = seed_piece(&app, "Haydn", "Sonata", "solo", 2);
    seed_session(&app, day, haydn, 0, 0, "auto-skipped");
    seed_session(&app, day, haydn, 25, 3, "");
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/day/{day}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("Bach"), "skipped card: {body}");
    assert!(body.contains("Vivaldi"), "auto skipped card: {body}");
    assert!(body.contains("Haydn"), "practiced entry: {body}");
    // The diary list groups the same sessions.
    let resp = r.clone().oneshot(req("GET", "/diary")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("Haydn"));
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary?c={next}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn piece_detail_score_and_availability_validation() {
    let (r, app) = test_app();
    let aid = seed_audition(&app, "Cup", "2027-01-10", 4);
    let bach = seed_piece(&app, "Bach", "Suite", "solo", 3);
    link(&app, bach, aid);
    let solo = seed_piece(&app, "Solo", "Etude", "solo", 1);
    // A piece in the plan shows its score panel; a piece without
    // auditions renders the plain page.
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/piece/{bach}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("Bach"));
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/piece/{solo}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // Availability validation: bad clock values and empty spans.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/availability",
            "date=2026-10-20&start=25:00&end=26:00&label=",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/availability",
            "date=2026-10-20&start=10:00&end=10:00&label=",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(form_req("/availability", "start=09:00&end=10:00&label=x"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req("/availability/delete", "id=0"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(req("GET", "/auditions/999/readiness"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    // Home in another language through the cookie.
    let localized = Request::builder()
        .method("GET")
        .uri("/")
        .header(header::COOKIE, "lang=de")
        .body(Body::empty())
        .unwrap();
    let resp = r.clone().oneshot(localized).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn recording_delete_flow() {
    let dir = std::env::temp_dir().join(format!("praeludium-test-del-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let app = Arc::new(App {
        db: Mutex::new(open_memory().unwrap()),
        data_dir: dir.clone(),
        current_user_id: praeludium::db::SEEDED_USER_ID,
    });
    let pid = seed_piece(&app, "Bach", "Suite", "solo", 3);
    let r = router(app);
    let boundary = "delboundary";
    let payload = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"t.webm\"\r\nContent-Type: audio/webm\r\n\r\nDATA\r\n--{boundary}--\r\n"
    );
    let upload = Request::builder()
        .method("POST")
        .uri(format!("/pieces/{pid}/recordings"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(payload))
        .unwrap();
    let resp = r.clone().oneshot(upload).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // Upload without the audio field is a 400.
    let payload2 = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"taken_on\"\r\n\r\n{}\r\n--{boundary}--\r\n",
        today()
    );
    let upload2 = Request::builder()
        .method("POST")
        .uri(format!("/pieces/{pid}/recordings"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(payload2))
        .unwrap();
    let resp = r.clone().oneshot(upload2).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(form_req("/recordings/1/delete", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(req("GET", "/recordings/1/audio"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn segment_link_time_flow_and_delete_guard() {
    use praeludium::db::Segment;
    let (r, app) = test_app();
    let roma = seed_audition(&app, "Roma", "2026-12-06", 4);
    let napoli = seed_audition(&app, "Napoli", "2026-12-06", 4);
    let pid = seed_piece(&app, "Haydn", "Concerto in Re", "solo", 3);

    // The segment is born on the piece and selected for the Roma link.
    let seg_id = app
        .add_segment(&Segment {
            piece_id: pid,
            alias: "Cadenza".into(),
            start_kind: "bar".into(),
            start_value: "100".into(),
            end_kind: "bar".into(),
            end_value: "130".into(),
            ..Default::default()
        })
        .unwrap();
    link(&app, pid, roma);
    link(&app, pid, napoli);
    app.set_link_segments(pid, roma, &[seg_id]).unwrap();
    let segs = app.segments_for_piece(pid).unwrap();
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].label, "Cadenza");
    assert_eq!(app.link_segment_ids(pid, roma).unwrap(), vec![seg_id]);

    // The audition program names the part; the piece page has
    // no segment management surface.
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{roma}")))
        .await
        .unwrap();
    assert!(body_string(resp).await.contains("Cadenza"));
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/piece/{pid}")))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("Cadenza"));
    assert!(!body.contains("segnew"));

    // Link Napoli to the same segment.
    app.set_link_segments(pid, napoli, &[seg_id]).unwrap();
    assert_eq!(app.link_segment_ids(pid, napoli).unwrap(), vec![seg_id]);

    // Roma drops to the whole piece: the segment survives
    // because Napoli still asks for it.
    app.set_link_segments(pid, roma, &[]).unwrap();
    assert!(app.link_segment_ids(pid, roma).unwrap().is_empty());
    assert_eq!(app.segments_for_piece(pid).unwrap().len(), 1);
    assert!(!app.delete_segment(seg_id).unwrap());

    // Napoli drops it too: no link references it, it deletes.
    app.set_link_segments(pid, napoli, &[]).unwrap();
    assert!(app.delete_segment(seg_id).unwrap());
    assert!(app.segments_for_piece(pid).unwrap().is_empty());
}

#[tokio::test]
async fn session_logging_with_segment_scope() {
    use praeludium::db::Segment;
    let (r, app) = test_app();
    let roma = seed_audition(&app, "Roma", "2026-12-06", 4);
    let pid = seed_piece(&app, "Haydn", "Concerto in Re", "solo", 3);
    let seg_id = app
        .add_segment(&Segment {
            piece_id: pid,
            alias: "Cadenza".into(),
            start_kind: "bar".into(),
            start_value: "100".into(),
            end_kind: "bar".into(),
            end_value: "130".into(),
            ..Default::default()
        })
        .unwrap();
    link(&app, pid, roma);
    app.set_link_segments(pid, roma, &[seg_id]).unwrap();

    // Log on the segment: the session carries it and the piece
    // page names it.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={pid}&minutes=25&confidence=4&segment_id={seg_id}"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    {
        let db = app.db.lock().unwrap();
        let stored: Option<i64> = db
            .query_row(
                "SELECT segment_id FROM sessions WHERE piece_id=?",
                rusqlite::params![pid],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, Some(seg_id));
    }
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/piece/{pid}")))
        .await
        .unwrap();
    assert!(body_string(resp).await.contains("Cadenza"));

    // A segment id of another piece falls back to whole piece.
    let bach = seed_piece(&app, "Bach", "Suite", "solo", 3);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={bach}&minutes=10&confidence=3&segment_id={seg_id}"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let db = app.db.lock().unwrap();
    let stored: Option<i64> = db
        .query_row(
            "SELECT segment_id FROM sessions WHERE piece_id=?",
            rusqlite::params![bach],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, None);
}

#[tokio::test]
async fn manual_session_date() {
    let (r, app) = test_app();
    let aid = seed_audition(&app, "Roma", "2026-12-06", 4);
    let pid = seed_piece(&app, "Bach", "Suite", "solo", 3);
    link(&app, pid, aid);

    // A chosen past date is stored as given.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={pid}&minutes=25&confidence=4&date=2026-08-15"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let stored: String = app
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT date FROM sessions WHERE piece_id=?",
            rusqlite::params![pid],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, "2026-08-15");
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary/day/2026-08-15"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // A skipped marker on the chosen date clears, as on today.
    seed_session(&app, "2026-08-16", pid, 0, 0, "skipped");
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={pid}&minutes=10&confidence=3&date=2026-08-16"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let markers: i64 = app
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE piece_id=? AND date='2026-08-16' AND note='skipped'",
            rusqlite::params![pid],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(markers, 0);

    // Malformed and future dates are rejected.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={pid}&minutes=10&confidence=3&date=2026-02-30"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={pid}&minutes=10&confidence=3&date=2999-01-01"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // No date still means today.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={pid}&minutes=10&confidence=3"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let stored: String = app
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT date FROM sessions WHERE piece_id=? AND minutes=10 AND confidence=3 AND date=?",
            rusqlite::params![pid, today()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, today());
}

#[tokio::test]
async fn today_plan_renders_segmented_items() {
    let (r, app) = test_app();
    let today = today();
    let roma = seed_audition(&app, "Roma", "2027-01-10", 4);
    let napoli = seed_audition(&app, "Napoli", "2027-02-10", 3);
    let pid = seed_piece(&app, "Haydn", "Concerto in Re", "solo", 3);
    link(&app, pid, roma);
    link(&app, pid, napoli);
    {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO segments (id, piece_id, alias) VALUES (30, ?, 'Cadenza')",
            rusqlite::params![pid],
        )
        .unwrap();
        db.execute(
            "INSERT INTO link_segments (piece_id, audition_id, segment_id) VALUES (?, ?, 30)",
            rusqlite::params![pid, roma],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, note) VALUES (?1, ?, 0, 3, 'baseline')",
            rusqlite::params![today, pid],
        )
        .unwrap();
        db.execute(
            "INSERT INTO availability (date, start_min, end_min, kind) VALUES (?1, 0, 1439, 'free')",
            rusqlite::params![today],
        )
        .unwrap();
    }
    declare_all_baselines(&app);
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    // Two items for one piece: the Cadenza scope for Roma and
    // the whole piece for Napoli. The segmented card names the
    // part and offers the segment choice in its log form.
    assert!(body.contains("Cadenza"), "segment label on the plan");
    assert!(body.contains("segment_id"), "segment select on the card");
    assert!(body.contains("Roma") && body.contains("Napoli"));
}

#[tokio::test]
async fn link_note_round_trip() {
    let (r, app) = test_app();
    let aid = seed_audition(&app, "Roma", "2026-12-06", 4);
    let pid = seed_piece(&app, "Haydn", "Concerto", "solo", 3);
    link_full(&app, pid, aid, "ed. Schott", false);
    assert_eq!(app.piece_auditions(pid).unwrap()[0].note, "ed. Schott");
    // The audition page and the piece page render the note as a note.
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}")))
        .await
        .unwrap();
    assert!(
        body_string(resp).await.contains("Note: ed. Schott"),
        "note on the audition page"
    );
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/piece/{pid}")))
        .await
        .unwrap();
    assert!(
        body_string(resp).await.contains("Note: ed. Schott"),
        "note on the piece page"
    );
}

#[tokio::test]
async fn mock_item_shows_extracts_and_note() {
    use praeludium::db::Segment;
    let (r, app) = test_app();
    let aid = seed_audition(&app, "Roma", "2026-12-06", 4);
    let pid = seed_piece(&app, "Haydn", "Concerto", "solo", 3);
    link_full(&app, pid, aid, "con cadenza", false);
    let seg_id = app
        .add_segment(&Segment {
            piece_id: pid,
            alias: "Cadenza".into(),
            start_kind: "bar".into(),
            start_value: "100".into(),
            end_kind: "bar".into(),
            end_value: "130".into(),
            ..Default::default()
        })
        .unwrap();
    app.set_link_segments(pid, aid, &[seg_id]).unwrap();
    let resp = r
        .clone()
        .oneshot(form_req(&format!("/auditions/{aid}/mock"), "count=1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}/mock/1")))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(
        body.contains("Excerpts: Cadenza"),
        "extract labels on the mock item"
    );
    assert!(
        body.contains("Note: con cadenza"),
        "link note on the mock item"
    );
}

#[tokio::test]
async fn diary_days_pagination() {
    let (r, app) = test_app();
    let aid = seed_audition(&app, "Roma", "2026-12-06", 4);
    let pid = seed_piece(&app, "Bach", "Suite", "solo", 3);
    link(&app, pid, aid);
    // 35 practiced days: three full pages of 10 and a final page of 5.
    for d in 1..=31 {
        seed_session(&app, &format!("2026-08-{d:02}"), pid, 30, 3, "");
    }
    for d in 1..=4 {
        seed_session(&app, &format!("2026-09-{d:02}"), pid, 30, 3, "");
    }
    let resp = r.clone().oneshot(req("GET", "/diary")).await.unwrap();
    let body = body_string(resp).await;
    assert_eq!(body.matches("/diary/day/").count(), 10, "page 1 day cards");
    assert!(
        body.contains("href=\"&#x2f;diary?page=2\">Next</a>"),
        "next href on page 1"
    );
    assert!(body.contains("Page 1 of 4"), "page label on page 1");
    assert!(
        !body.contains("/diary/day/2026-08-01"),
        "oldest day not on page 1"
    );
    // The audition selection rides along in the pager hrefs.
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary?c={aid}")))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(
        body.contains(&format!("href=\"&#x2f;diary?c={aid}&amp;page=2\">Next</a>")),
        "filtered next href"
    );
    // Page 2: another full page, prev back to the plain URL.
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary?page=2"))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert_eq!(body.matches("/diary/day/").count(), 10, "page 2 day cards");
    assert!(
        body.contains("href=\"&#x2f;diary\">Previous</a>"),
        "prev href on page 2"
    );
    assert!(
        body.contains("href=\"&#x2f;diary?page=3\">Next</a>"),
        "next href on page 2"
    );
    assert!(body.contains("Page 2 of 4"), "page label on page 2");
    // Page 4: the remaining 5 days, including the oldest day.
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary?page=4"))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert_eq!(body.matches("/diary/day/").count(), 5, "page 4 day cards");
    assert!(
        body.contains("/diary/day/2026-08-01"),
        "oldest day on page 4"
    );
    assert!(
        body.contains("href=\"&#x2f;diary?page=3\">Previous</a>"),
        "prev href on page 4"
    );
    assert!(!body.contains("Next</a>"), "no next on the last page");
    assert!(body.contains("Page 4 of 4"), "page label on page 4");
    // Out-of-range pages clamp: 99 to the last, 0 to the first.
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary?page=99"))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert_eq!(
        body.matches("/diary/day/").count(),
        5,
        "clamped to last page"
    );
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary?page=0"))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert_eq!(
        body.matches("/diary/day/").count(),
        10,
        "page 0 clamps to first"
    );
}

#[tokio::test]
async fn link_optional_round_trip() {
    let (r, app) = test_app();
    let aid = seed_audition(&app, "San Carlo", "2027-01-10", 4);
    // The piece links as an optional choice.
    let pid = seed_piece(&app, "Dvorak", "Concerto", "solo", 3);
    link_full(&app, pid, aid, "", true);
    assert!(app.piece_auditions(pid).unwrap()[0].optional);
    // The audition page and the piece page render the badge.
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}")))
        .await
        .unwrap();
    assert!(
        body_string(resp).await.contains("Optional"),
        "badge on the audition page"
    );
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/diary/piece/{pid}")))
        .await
        .unwrap();
    assert!(
        body_string(resp).await.contains("Optional"),
        "badge on the piece page"
    );
    // A piece linked without the flag defaults to required.
    let haydn = seed_piece(&app, "Haydn", "Concerto", "solo", 3);
    link(&app, haydn, aid);
    assert!(!app.piece_auditions(haydn).unwrap()[0].optional);
}

#[tokio::test]
async fn pool_card_on_audition_page_and_prepmap() {
    let (r, app) = test_app();
    let aid = seed_audition(&app, "San Carlo", "2027-01-10", 4);
    let rid = seed_round(&app, aid, "Finale");
    let dvorak = seed_piece(&app, "Dvorak", "Concerto", "solo", 3);
    let schumann = seed_piece(&app, "Schumann", "Concerto", "solo", 3);
    let elgar = seed_piece(&app, "Elgar", "Concerto", "solo", 3);
    let haydn = seed_piece(&app, "Haydn", "Concerto in Re", "solo", 3);
    {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO piece_audition (piece_id, audition_id, round_id, optional) VALUES (?, ?, ?, 1), (?, ?, ?, 1), (?, ?, ?, 1), (?, ?, ?, 0)",
            rusqlite::params![dvorak, aid, rid, schumann, aid, rid, elgar, aid, rid, haydn, aid, rid],
        )
        .unwrap();
    }
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    // Four linked pieces render as two cards: one pool card
    // with the three alternatives, one single card.
    assert_eq!(
        body.matches("<article class=\"card reveal").count(),
        2,
        "pool members share one card"
    );
    // The pool card is composite: one subcard per member, each
    // with its own head link, inside a single outer card.
    assert_eq!(
        body.matches("<div class=\"subcard\">").count(),
        3,
        "pool card has one subcard per member"
    );
    for id in [dvorak, schumann, elgar] {
        assert!(
            body.contains(&format!("href=\"/diary/piece/{id}\"")),
            "pool subcard links member {id}"
        );
    }
    assert!(body.contains("Optional"), "optional badge on the page");
    // The preparation map carries one tile for the pool,
    // labelled with every member.
    assert!(
        body.contains("Dvorak — Concerto &#x2f; Elgar — Concerto &#x2f; Schumann — Concerto"),
        "pool tile lists every member"
    );
}

#[tokio::test]
async fn optional_tile_badge_audition_and_diary_scope() {
    let (r, app) = test_app();
    let carlo = seed_audition(&app, "San Carlo", "2027-01-10", 4);
    let roma = seed_audition(&app, "Roma", "2027-01-20", 4);
    let dvorak = seed_piece(&app, "Dvorak", "Concerto", "solo", 3);
    let haydn = seed_piece(&app, "Haydn", "Concerto in Re", "solo", 3);
    // Dvorak is a choice for San Carlo but mandatory for Roma.
    link_full(&app, dvorak, carlo, "", true);
    link_full(&app, dvorak, roma, "", false);
    link_full(&app, haydn, carlo, "", false);
    // Audition map: the choice tile carries the badge in both
    // layouts, the mandatory tile does not.
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{carlo}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert_eq!(
        body.matches("ptile-badge").count(),
        2,
        "audition map badges the choice tile in both layouts"
    );
    // Diary map: no badge unfiltered, or for Roma where the
    // piece is mandatory; badge only for the single San Carlo view.
    for (path, badges) in [
        ("/diary".to_string(), 0),
        (format!("/diary?c={roma}"), 0),
        (format!("/diary?c={carlo},{roma}"), 0),
        (format!("/diary?c={carlo}"), 2),
    ] {
        let resp = r.clone().oneshot(req("GET", &path)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await;
        assert_eq!(
            body.matches("ptile-badge").count(),
            badges,
            "diary map badge count for {path}"
        );
    }
}

#[tokio::test]
async fn pool_plan_card_shared_weight_and_member_choice() {
    let (r, app) = test_app();
    let today = today();
    let aid = seed_audition(&app, "San Carlo", "2027-01-10", 4);
    let dvorak = seed_piece(&app, "Dvorak", "Concerto", "solo", 3);
    let schumann = seed_piece(&app, "Schumann", "Concerto", "solo", 3);
    link_full(&app, dvorak, aid, "", true);
    link_full(&app, schumann, aid, "", true);
    {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, note) VALUES (?1, ?, 0, 3, 'baseline'), (?1, ?, 0, 3, 'baseline')",
            rusqlite::params![today, dvorak, schumann],
        )
        .unwrap();
        db.execute(
            "INSERT INTO availability (date, start_min, end_min, kind) VALUES (?1, 0, 1439, 'free')",
            rusqlite::params![today],
        )
        .unwrap();
    }
    declare_all_baselines(&app);
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    // One plan card for the pool, with a member selector in
    // its log form next to the manual form's own selector.
    assert_eq!(
        body.matches("<article class=\"card plan-card reveal\">")
            .count(),
        1,
        "pool members share one plan card"
    );
    assert_eq!(
        body.matches("<select name=\"piece_id\"").count(),
        2,
        "pool card carries a member selector"
    );
    // The pool counts the audition weight once, not per member.
    assert!(body.contains("base 4.0"), "shared weight");
    assert!(!body.contains("base 8.0"), "weight not summed per member");
    // Logging through the pool card records on the picked
    // member only.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={schumann}&minutes=20&confidence=4"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let db = app.db.lock().unwrap();
    let logged: i64 = db
        .query_row(
            "SELECT COALESCE(SUM(minutes), 0) FROM sessions WHERE piece_id=?1 AND date=?2 AND minutes>0",
            rusqlite::params![schumann, today],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(logged, 20, "session on the picked member");
    let other: i64 = db
        .query_row(
            "SELECT COALESCE(SUM(minutes), 0) FROM sessions WHERE piece_id=?1 AND date=?2 AND minutes>0",
            rusqlite::params![dvorak, today],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(other, 0, "no session on the other member");
}

#[tokio::test]
async fn pool_member_with_mandatory_link_planned_normally() {
    let (r, app) = test_app();
    let today = today();
    let carlo = seed_audition(&app, "San Carlo", "2027-01-10", 4);
    let cagliari = seed_audition(&app, "Cagliari", "2027-02-10", 3);
    let dvorak = seed_piece(&app, "Dvorak", "Concerto", "solo", 3);
    let schumann = seed_piece(&app, "Schumann", "Concerto", "solo", 3);
    link_full(&app, dvorak, carlo, "", true);
    link_full(&app, dvorak, cagliari, "", false);
    link_full(&app, schumann, carlo, "", true);
    {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, note) VALUES (?1, ?, 0, 3, 'baseline'), (?1, ?, 0, 3, 'baseline')",
            rusqlite::params![today, dvorak, schumann],
        )
        .unwrap();
        db.execute(
            "INSERT INTO availability (date, start_min, end_min, kind) VALUES (?1, 0, 1439, 'free')",
            rusqlite::params![today],
        )
        .unwrap();
    }
    declare_all_baselines(&app);
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    // The mandatory-elsewhere member leaves the pool: two
    // single cards, no pool selector beyond the manual form.
    assert_eq!(
        body.matches("<article class=\"card plan-card reveal\">")
            .count(),
        2,
        "no pool card when a member is mandatory elsewhere"
    );
    assert_eq!(
        body.matches("<select name=\"piece_id\"").count(),
        1,
        "no pool member selector"
    );
}

#[tokio::test]
async fn enrollment_toggle_round_trip() {
    let (r, app) = test_app();
    let aid = seed_open_audition(&app, "Cup", "2027-01-10", 4);
    // A new audition starts unenrolled: no badge, enroll button.
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}")))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(!body.contains("Enrolled"), "starts unenrolled: {body}");
    assert!(body.contains("Enroll"), "enroll button shows");
    // Enroll: the row appears and the badge flips to Unenroll.
    let resp = r
        .clone()
        .oneshot(form_req(&format!("/auditions/{aid}/enrollment"), ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        enrollment_count(&app, aid),
        1,
        "one enrollments row after enroll"
    );
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}")))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("Enrolled"), "badge shows after enroll");
    assert!(body.contains("Unenroll"), "unenroll button shows");
    // Unenroll: the row is gone and the badge with it.
    let resp = r
        .clone()
        .oneshot(form_req(&format!("/auditions/{aid}/enrollment"), ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(enrollment_count(&app, aid), 0, "no row after unenroll");
    let resp = r
        .clone()
        .oneshot(req("GET", &format!("/auditions/{aid}")))
        .await
        .unwrap();
    assert!(!body_string(resp).await.contains("Enrolled"));
    // An unknown audition is a 404, not a flip.
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/999/enrollment", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn enrollment_toggle_ensures_user_on_fresh_database() {
    let (r, app) = test_app();
    // A fresh database holds no users row (Phase 1).
    let users: i64 = {
        let db = app.db.lock().unwrap();
        db.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .unwrap()
    };
    assert_eq!(users, 0, "fresh database holds no users row");
    let audition_id = {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO auditions(name, audition_date, weight) VALUES('Fresh', '2027-03-01', 3)",
            [],
        )
        .unwrap();
        db.last_insert_rowid()
    };
    let resp = r
        .clone()
        .oneshot(form_req(
            &format!("/auditions/{audition_id}/enrollment"),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        enrollment_count(&app, audition_id),
        1,
        "one enrollments row after enroll"
    );
}

fn enrollment_count(app: &App, audition_id: i64) -> i64 {
    let db = app.db.lock().unwrap();
    db.query_row(
        "SELECT COUNT(*) FROM enrollments WHERE user_id=1 AND audition_id=?",
        rusqlite::params![audition_id],
        |row| row.get(0),
    )
    .unwrap()
}

#[tokio::test]
async fn plan_excludes_unenrolled_and_diary_stays_open() {
    let (r, app) = test_app();
    let aid = seed_open_audition(&app, "Cup", "2027-01-10", 4);
    let pid = seed_piece(&app, "Bach", "Suite", "solo", 3);
    link(&app, pid, aid);
    r.clone()
        .oneshot(form_req(&format!("/pieces/{pid}/baseline"), "confidence=4"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/availability",
            &format!("date={}&start=08:00&end=23:00&label=", today()),
        ))
        .await
        .unwrap();
    declare_all_baselines(&app);
    // Unenrolled: the piece is linked but the plan is empty.
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    let body = body_string(resp).await;
    assert_eq!(
        body.matches("<article class=\"card plan-card reveal\">")
            .count(),
        0,
        "no plan card while unenrolled"
    );
    // Diary logging still works on the unenrolled piece. The
    // session is dated in the past, so today stays unpracticed
    // and the plan card below still renders as a plan item.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id={pid}&minutes=10&confidence=3&date=2026-01-05"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let logged: i64 = {
        let db = app.db.lock().unwrap();
        db.query_row(
            "SELECT COUNT(*) FROM sessions WHERE piece_id=? AND minutes=10",
            rusqlite::params![pid],
            |row| row.get(0),
        )
        .unwrap()
    };
    assert_eq!(logged, 1, "session saved while unenrolled");
    // Enroll: the same piece reaches the plan.
    r.clone()
        .oneshot(form_req(&format!("/auditions/{aid}/enrollment"), ""))
        .await
        .unwrap();
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    let body = body_string(resp).await;
    assert_eq!(
        body.matches("<article class=\"card plan-card reveal\">")
            .count(),
        1,
        "one plan card after enroll"
    );
}

#[tokio::test]
async fn manual_log_list_shows_enrolled_pieces_only() {
    let (r, app) = test_app();
    let cup = seed_open_audition(&app, "Cup", "2027-01-10", 4);
    let plate = seed_open_audition(&app, "Plate", "2027-02-10", 3);
    let bach = seed_piece(&app, "Bach", "Suite", "solo", 3);
    link(&app, bach, cup);
    let haydn = seed_piece(&app, "Haydn", "Concerto", "solo", 3);
    link(&app, haydn, plate);
    // Enroll only in Cup: the manual log list keeps the Cup piece
    // and drops the Plate piece.
    r.clone()
        .oneshot(form_req(&format!("/auditions/{cup}/enrollment"), ""))
        .await
        .unwrap();
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    let body = body_string(resp).await;
    let form = body
        .split_once("<form method=\"post\" action=\"/session\" class=\"checkin\">")
        .expect("checkin form")
        .1
        .split_once("</form>")
        .expect("checkin form end")
        .0;
    assert!(
        form.contains(&format!("<option value=\"{bach}\">")),
        "enrolled piece listed"
    );
    assert!(
        !form.contains(&format!("<option value=\"{haydn}\">")),
        "unenrolled piece hidden"
    );
}
