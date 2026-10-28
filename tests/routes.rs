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
    });
    let r = router(app.clone());
    (r, app)
}

fn test_router() -> axum::Router {
    test_app().0
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
}

#[tokio::test]
async fn piece_and_session_flow() {
    let r = test_router();
    let resp = r
        .clone()
        .oneshot(form_req(
            "/auditions",
            "name=Test+Audition&date=2027-01-10&weight=4",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Bach&work=Suite&movement=&kind=solo&difficulty=3&audition=1",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r.clone().oneshot(req("GET", "/pieces")).await.unwrap();
    assert!(body_string(resp).await.contains("Bach"));
    // The piece waits for its baseline declaration on the home page.
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    assert!(body_string(resp).await.contains("baseline"));
    let resp = r
        .clone()
        .oneshot(form_req("/pieces/1/baseline", "confidence=3"))
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
        .oneshot(form_req("/session", "piece_id=1&minutes=30&confidence=4"))
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
        .oneshot(req("GET", "/diary/piece/1"))
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
        .oneshot(form_req("/pieces", "composer=&work="))
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
        .oneshot(req("GET", "/diary/piece/42"))
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
    let r = test_router();
    r.clone()
        .oneshot(form_req(
            "/auditions",
            "name=Mock+Cup&date=2027-02-01&weight=3",
        ))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Brahms&work=Sonata&kind=excerpt&difficulty=5&audition=1",
        ))
        .await
        .unwrap();
    let resp = r
        .clone()
        .oneshot(req("GET", "/auditions/1/mock"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/mock", "count=1&record=1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).unwrap(),
        "/auditions/1/mock/1"
    );
    let resp = r
        .clone()
        .oneshot(req("GET", "/auditions/1/mock/1"))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("Brahms"), "mock run shows the piece");
    let resp = r
        .clone()
        .oneshot(form_req(
            "/auditions/1/mock/1/rate",
            "item_id=1&confidence=5&note=ok",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(req("GET", "/auditions/1/mock/1"))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("5/5") || body.contains("5,5") || body.contains('5'));
    let resp = r
        .clone()
        .oneshot(req("GET", "/auditions/1/readiness"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/mock/1/delete", ""))
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
    });
    let r = router(app);
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Bach&work=Suite&kind=solo&difficulty=3",
        ))
        .await
        .unwrap();
    let boundary = "testboundary42";
    let payload = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"taken_on\"\r\n\r\n{}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"duration_sec\"\r\n\r\n12\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"take.webm\"\r\nContent-Type: audio/webm\r\n\r\nWEBMDATA0123456789\r\n--{boundary}--\r\n",
        today()
    );
    let upload = Request::builder()
        .method("POST")
        .uri("/pieces/1/recordings")
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
        .oneshot(req("GET", "/diary/piece/1"))
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
async fn audition_links_and_rounds_crud() {
    let r = test_router();
    r.clone()
        .oneshot(form_req(
            "/auditions",
            "name=Cup&date=2027-01-10&weight=4&link_url=https://example.org/bando&link_label=",
        ))
        .await
        .unwrap();
    let resp = r.clone().oneshot(req("GET", "/auditions/1")).await.unwrap();
    let body = body_string(resp).await;
    assert!(
        body.contains("example.org"),
        "detail lists the link host label"
    );
    let resp = r
        .clone()
        .oneshot(req("GET", "/auditions/1/edit"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/auditions/1/links",
            "link_url=https://example.org/parti.pdf&link_label=Parti",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/links", "link_url="))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/links/2/delete", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/rounds", "round_name=Eliminatoria"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/rounds", "round_name="))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/rounds/1/delete", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/auditions/1",
            "name=Cup+Final&date=2027-01-11&weight=5&return=/auditions/1",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req("/auditions", "name=Bad&date=&weight=3"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
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
    // Archive and restore through the detail return path.
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/archive", "return=/auditions/1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/restore", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn piece_edit_archive_and_filters() {
    let r = test_router();
    r.clone()
        .oneshot(form_req("/auditions", "name=Cup&date=2027-01-10&weight=4"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req("/auditions/1/rounds", "round_name=Finale"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Bach&work=Suite&kind=solo&difficulty=3&audition=1&round-1=1",
        ))
        .await
        .unwrap();
    let resp = r
        .clone()
        .oneshot(req("GET", "/pieces/1/edit"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/pieces/1/edit",
            "composer=Bach&work=Suite+II&movement=Preludio&kind=excerpt&difficulty=4&target_bpm=72&audition=1&note-1=da+capo&round-1=1",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(req("GET", "/pieces?audition=1&kind=excerpt&q=Suite"))
        .await
        .unwrap();
    assert!(body_string(resp).await.contains("Suite II"));
    let resp = r
        .clone()
        .oneshot(req("GET", "/pieces?q=nomatch"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = r
        .clone()
        .oneshot(form_req("/pieces/1/archive", "return=/diary/piece/1"))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get(header::LOCATION).unwrap(),
        "/diary/piece/1"
    );
    let resp = r
        .clone()
        .oneshot(req("GET", "/pieces?archiviati=1"))
        .await
        .unwrap();
    assert!(body_string(resp).await.contains("Suite II"));
    let resp = r
        .clone()
        .oneshot(form_req("/pieces/1/restore", ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
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
    r.clone()
        .oneshot(form_req("/auditions", "name=Cup&date=2027-01-10&weight=4"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Bach&work=Suite&kind=solo&difficulty=3&audition=1",
        ))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req("/pieces/1/baseline", "confidence=4"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/availability",
            &format!("date={}&start=08:00&end=23:00&label=", today()),
        ))
        .await
        .unwrap();
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("plan-card"), "plan shows the piece: {body}");
    let resp = r
        .clone()
        .oneshot(form_req("/session/skip", "piece_id=1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("skipped-card"), "home shows the skip card");
    // Logging minutes after the skip clears the marker.
    let resp = r
        .clone()
        .oneshot(form_req("/session", "piece_id=1&minutes=10&confidence=3"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r.clone().oneshot(req("GET", "/")).await.unwrap();
    assert!(!body_string(resp).await.contains("skipped-card"));
    // The diary day page groups and labels the day.
    seed_session(&app, &today(), 1, 0, 0, "postponed");
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
            "piece_id=1&minutes=5&confidence=3&bpm=999",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn settings_save_and_diary_filter() {
    let r = test_router();
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
    r.clone()
        .oneshot(form_req("/auditions", "name=Cup&date=2027-01-10&weight=4"))
        .await
        .unwrap();
    let resp = r.clone().oneshot(req("GET", "/diary?c=1")).await.unwrap();
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
    let r = test_router();
    r.clone()
        .oneshot(form_req("/auditions", "name=Cup&date=2027-01-10&weight=4"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req("/auditions/1/rounds", "round_name=Finale"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Bach&work=Suite&kind=solo&difficulty=3&audition=1&round-1=1",
        ))
        .await
        .unwrap();
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/mock", "round_id=99"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/mock", "round_id=1&count=5"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(form_req(
            "/auditions/1/mock/1/rate",
            "item_id=1&confidence=2",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    // Second rating of the same item is a no-op by design.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/auditions/1/mock/1/rate",
            "item_id=1&confidence=5",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r.clone().oneshot(req("GET", "/auditions/1")).await.unwrap();
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
    r.clone()
        .oneshot(form_req(
            "/auditions",
            "name=Past+Cup&date=2020-01-10&weight=2",
        ))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/auditions",
            "name=Next+Cup&date=2027-01-10&weight=4&link_url=https://example.org/x&link_label=Bando",
        ))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Bach&work=Suite&kind=solo&difficulty=3&audition=2",
        ))
        .await
        .unwrap();
    let resp = r.clone().oneshot(req("GET", "/auditions")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("Past Cup") && body.contains("Next Cup"));
    // Diary day with every marker kind on a fixed past date.
    let day = "2026-10-12";
    seed_session(&app, day, 1, 0, 0, "skipped");
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Vivaldi&work=Concerto&kind=solo&difficulty=2",
        ))
        .await
        .unwrap();
    seed_session(&app, day, 2, 0, 0, "auto-skipped");
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Haydn&work=Sonata&kind=solo&difficulty=2",
        ))
        .await
        .unwrap();
    seed_session(&app, day, 3, 0, 0, "auto-skipped");
    seed_session(&app, day, 3, 25, 3, "");
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
    let resp = r.clone().oneshot(req("GET", "/diary?c=2")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn piece_detail_score_and_availability_validation() {
    let r = test_router();
    r.clone()
        .oneshot(form_req("/auditions", "name=Cup&date=2027-01-10&weight=4"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Bach&work=Suite&kind=solo&difficulty=3&audition=1",
        ))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Solo&work=Etude&kind=solo&difficulty=1",
        ))
        .await
        .unwrap();
    // A piece in the plan shows its score panel; a piece without
    // auditions renders the plain page.
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary/piece/1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("Bach"));
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary/piece/2"))
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
    });
    let r = router(app);
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Bach&work=Suite&kind=solo&difficulty=3",
        ))
        .await
        .unwrap();
    let boundary = "delboundary";
    let payload = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"t.webm\"\r\nContent-Type: audio/webm\r\n\r\nDATA\r\n--{boundary}--\r\n"
    );
    let upload = Request::builder()
        .method("POST")
        .uri("/pieces/1/recordings")
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
        .uri("/pieces/1/recordings")
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
async fn segment_link_time_flow_and_orphan_gc() {
    let (r, app) = test_app();
    for name in ["Roma", "Napoli"] {
        r.clone()
            .oneshot(form_req(
                "/auditions",
                &format!("name={name}&date=2026-12-06&weight=4"),
            ))
            .await
            .unwrap();
    }

    // Add the piece to Roma with an inline new segment: the
    // segment is born on the piece and selected for the link.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Haydn&work=Concerto+in+Re&kind=solo&difficulty=3&audition=1&segnew-alias-1=Cadenza&segnew-start-kind-1=bar&segnew-start-value-1=100&segnew-end-kind-1=bar&segnew-end-value-1=130",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let segs = app.segments_for_piece(1).unwrap();
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].label, "Cadenza");
    let seg_id = segs[0].id;
    assert_eq!(app.link_segment_ids(1, 1).unwrap(), vec![seg_id]);

    // The audition program names the part; the piece page has
    // no segment management surface.
    let resp = r.clone().oneshot(req("GET", "/auditions/1")).await.unwrap();
    assert!(body_string(resp).await.contains("Cadenza"));
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary/piece/1"))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("Cadenza"));
    assert!(!body.contains("segnew"));

    // Link Napoli to the same segment through the edit form.
    r.clone()
        .oneshot(form_req(
            "/pieces/1/edit",
            &format!("composer=Haydn&work=Concerto+in+Re&kind=solo&difficulty=3&audition=1&audition=2&segment-1={seg_id}&segment-2={seg_id}"),
        ))
        .await
        .unwrap();
    assert_eq!(app.link_segment_ids(1, 2).unwrap(), vec![seg_id]);

    // Roma drops to the whole piece: the segment survives
    // because Napoli still asks for it.
    r.clone()
        .oneshot(form_req(
            "/pieces/1/edit",
            &format!("composer=Haydn&work=Concerto+in+Re&kind=solo&difficulty=3&audition=1&audition=2&segment-2={seg_id}"),
        ))
        .await
        .unwrap();
    assert!(app.link_segment_ids(1, 1).unwrap().is_empty());
    assert_eq!(app.segments_for_piece(1).unwrap().len(), 1);

    // Napoli drops it too: no link references it, it is gone.
    r.clone()
        .oneshot(form_req(
            "/pieces/1/edit",
            "composer=Haydn&work=Concerto+in+Re&kind=solo&difficulty=3&audition=1&audition=2",
        ))
        .await
        .unwrap();
    assert!(app.segments_for_piece(1).unwrap().is_empty());

    // The same inline definition twice creates one segment.
    for _ in 0..2 {
        r.clone()
            .oneshot(form_req(
                "/pieces/1/edit",
                "composer=Haydn&work=Concerto+in+Re&kind=solo&difficulty=3&audition=1&segnew-alias-1=Cadenza&segnew-start-kind-1=bar&segnew-start-value-1=100&segnew-end-kind-1=bar&segnew-end-value-1=130",
            ))
            .await
            .unwrap();
    }
    assert_eq!(app.segments_for_piece(1).unwrap().len(), 1);
    assert_eq!(app.link_segment_ids(1, 1).unwrap().len(), 1);
}

#[tokio::test]
async fn session_logging_with_segment_scope() {
    let (r, app) = test_app();
    r.clone()
        .oneshot(form_req("/auditions", "name=Roma&date=2026-12-06&weight=4"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Haydn&work=Concerto+in+Re&kind=solo&difficulty=3&audition=1&segnew-alias-1=Cadenza&segnew-start-kind-1=bar&segnew-start-value-1=100&segnew-end-kind-1=bar&segnew-end-value-1=130",
        ))
        .await
        .unwrap();
    let seg_id = app.segments_for_piece(1).unwrap()[0].id;

    // Log on the segment: the session carries it and the piece
    // page names it.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id=1&minutes=25&confidence=4&segment_id={seg_id}"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    {
        let db = app.db.lock().unwrap();
        let stored: Option<i64> = db
            .query_row(
                "SELECT segment_id FROM sessions WHERE piece_id=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, Some(seg_id));
    }
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary/piece/1"))
        .await
        .unwrap();
    assert!(body_string(resp).await.contains("Cadenza"));

    // A segment id of another piece falls back to whole piece.
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Bach&work=Suite&kind=solo&difficulty=3",
        ))
        .await
        .unwrap();
    let resp = r
        .clone()
        .oneshot(form_req(
            "/session",
            &format!("piece_id=2&minutes=10&confidence=3&segment_id={seg_id}"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let db = app.db.lock().unwrap();
    let stored: Option<i64> = db
        .query_row(
            "SELECT segment_id FROM sessions WHERE piece_id=2",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, None);
}

#[tokio::test]
async fn today_plan_renders_segmented_items() {
    let (r, app) = test_app();
    let today = today();
    {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO auditions (id, name, audition_date, weight) VALUES (1, 'Roma', '2027-01-10', 4), (2, 'Napoli', '2027-02-10', 3)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO pieces (id, composer, work, movement, kind, difficulty) VALUES (1, 'Haydn', 'Concerto in Re', 'I mov.', 'solo', 3)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO piece_audition (piece_id, audition_id) VALUES (1, 1), (1, 2)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO segments (id, piece_id, alias) VALUES (10, 1, 'Cadenza')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO link_segments (piece_id, audition_id, segment_id) VALUES (1, 1, 10)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, note) VALUES (?1, 1, 0, 3, 'baseline')",
            rusqlite::params![today],
        )
        .unwrap();
        db.execute(
            "INSERT INTO availability (date, start_min, end_min, kind) VALUES (?1, 0, 1439, 'free')",
            rusqlite::params![today],
        )
        .unwrap();
    }
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
    r.clone()
        .oneshot(form_req("/auditions", "name=Roma&date=2026-12-06&weight=4"))
        .await
        .unwrap();
    // Add the piece with a note on the link.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Haydn&work=Concerto&kind=solo&difficulty=3&audition=1&note-1=ed.+Schott",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(app.piece_auditions(1).unwrap()[0].note, "ed. Schott");
    // The edit form carries the note and no requirement text input.
    let resp = r
        .clone()
        .oneshot(req("GET", "/pieces/1/edit"))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert!(body.contains("name=\"note-1\""), "note input on the form");
    assert!(body.contains("ed. Schott"), "note value on the form");
    assert!(
        !body.contains("name=\"excerpt-1\""),
        "no requirement text input"
    );
    // An unrelated edit that carries the note keeps it.
    let resp = r
        .clone()
        .oneshot(form_req(
            "/pieces/1/edit",
            "composer=Haydn&work=Concerto&movement=I+mov.&kind=solo&difficulty=3&audition=1&note-1=ed.+Schott",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(app.piece_auditions(1).unwrap()[0].note, "ed. Schott");
    // The audition page and the piece page render the note as a note.
    let resp = r.clone().oneshot(req("GET", "/auditions/1")).await.unwrap();
    assert!(
        body_string(resp).await.contains("Note: ed. Schott"),
        "note on the audition page"
    );
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary/piece/1"))
        .await
        .unwrap();
    assert!(
        body_string(resp).await.contains("Note: ed. Schott"),
        "note on the piece page"
    );
}

#[tokio::test]
async fn mock_item_shows_extracts_and_note() {
    let r = test_router();
    r.clone()
        .oneshot(form_req("/auditions", "name=Roma&date=2026-12-06&weight=4"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Haydn&work=Concerto&kind=solo&difficulty=3&audition=1&note-1=con+cadenza&segnew-alias-1=Cadenza&segnew-start-kind-1=bar&segnew-start-value-1=100&segnew-end-kind-1=bar&segnew-end-value-1=130",
        ))
        .await
        .unwrap();
    let resp = r
        .clone()
        .oneshot(form_req("/auditions/1/mock", "count=1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = r
        .clone()
        .oneshot(req("GET", "/auditions/1/mock/1"))
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
    r.clone()
        .oneshot(form_req("/auditions", "name=Roma&date=2026-12-06&weight=4"))
        .await
        .unwrap();
    r.clone()
        .oneshot(form_req(
            "/pieces",
            "composer=Bach&work=Suite&kind=solo&difficulty=3&audition=1",
        ))
        .await
        .unwrap();
    // 35 practiced days: page 1 holds 30, page 2 the rest.
    for d in 1..=31 {
        seed_session(&app, &format!("2026-08-{d:02}"), 1, 30, 3, "");
    }
    for d in 1..=4 {
        seed_session(&app, &format!("2026-09-{d:02}"), 1, 30, 3, "");
    }
    let resp = r.clone().oneshot(req("GET", "/diary")).await.unwrap();
    let body = body_string(resp).await;
    assert_eq!(body.matches("/diary/day/").count(), 30, "page 1 day cards");
    assert!(
        body.contains("href=\"&#x2f;diary?page=2\">Next</a>"),
        "next href on page 1"
    );
    assert!(body.contains("Page 1 of 2"), "page label on page 1");
    assert!(
        !body.contains("/diary/day/2026-08-01"),
        "oldest day not on page 1"
    );
    // The audition selection rides along in the pager hrefs.
    let resp = r.clone().oneshot(req("GET", "/diary?c=1")).await.unwrap();
    let body = body_string(resp).await;
    assert!(
        body.contains("href=\"&#x2f;diary?c=1&amp;page=2\">Next</a>"),
        "filtered next href"
    );
    // Page 2: the remaining 5 days, prev back to the plain URL.
    let resp = r
        .clone()
        .oneshot(req("GET", "/diary?page=2"))
        .await
        .unwrap();
    let body = body_string(resp).await;
    assert_eq!(body.matches("/diary/day/").count(), 5, "page 2 day cards");
    assert!(
        body.contains("/diary/day/2026-08-01"),
        "oldest day on page 2"
    );
    assert!(
        body.contains("href=\"&#x2f;diary\">Previous</a>"),
        "prev href on page 2"
    );
    assert!(!body.contains("Next</a>"), "no next on the last page");
    assert!(body.contains("Page 2 of 2"), "page label on page 2");
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
        30,
        "page 0 clamps to first"
    );
}
