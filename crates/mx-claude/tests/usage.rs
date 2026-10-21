use axum::Router;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use chrono::{Local, TimeDelta, TimeZone, Utc};
use mx_claude::usage::{
    EXPIRED, NO_TOKEN, OFFLINE, UsageAccount, access_token, account, fetch_usage, parse,
    short_until,
};

const SAMPLE: &str = r#"
{
  "limits": [
    {"kind": "session", "group": "session", "percent": 3, "severity": "normal",
     "resets_at": "2026-09-10T16:09:59.634925+00:00", "scope": null, "is_active": false},
    {"kind": "weekly_all", "group": "weekly", "percent": 18, "severity": "normal",
     "resets_at": "2026-09-13T19:59:59.634945+00:00", "scope": null, "is_active": true},
    {"kind": "weekly_scoped", "group": "weekly", "percent": 14, "severity": "normal",
     "resets_at": "2026-09-13T19:59:59.635165+00:00",
     "scope": {"model": {"id": null, "display_name": "Fable"}, "surface": null},
     "is_active": false}
  ],
  "spend": {"used": {"amount_minor": 0, "currency": "BRL", "exponent": 2},
            "limit": null, "percent": 0, "enabled": false}
}
"#;

fn read(body: &str) -> mx_claude::usage::UsageReading {
    parse(body, UsageAccount::default())
}

#[test]
fn the_three_plan_windows_in_api_order() {
    let reading = read(SAMPLE);
    assert!(reading.ok(), "{:?}", reading.problem);
    let labels: Vec<&str> = reading.windows.iter().map(|w| w.label.as_str()).collect();
    assert_eq!(labels, ["Sessão (5h)", "Semanal (7 dias)", "Semanal Fable"]);
    let percents: Vec<i64> = reading.windows.iter().map(|w| w.percent).collect();
    assert_eq!(percents, [3, 18, 14]);
    assert_eq!(reading.credits, None);
}

#[test]
fn reset_becomes_the_same_instant_it_came_as() {
    let session = &read(SAMPLE).windows[0];
    let at = session.resets_at.unwrap().with_timezone(&Utc);
    let expected =
        Utc.with_ymd_and_hms(2026, 9, 10, 16, 9, 59).unwrap() + TimeDelta::microseconds(634925);
    assert_eq!(at, expected);
}

#[test]
fn an_unknown_window_comes_in_anyway() {
    let reading =
        read(r#"{"limits": [{"kind": "monthly_whatever", "percent": 7, "resets_at": null}]}"#);
    assert!(reading.ok());
    assert_eq!(reading.windows[0].label, "monthly whatever");
    assert_eq!(reading.windows[0].percent, 7);
    assert_eq!(reading.windows[0].resets_at, None);
}

#[test]
fn a_window_without_percent_is_no_bar() {
    let reading = read(
        r#"{"limits": [{"kind": "session", "percent": null}, {"kind": "weekly_all", "percent": 18}]}"#,
    );
    let percents: Vec<i64> = reading.windows.iter().map(|w| w.percent).collect();
    assert_eq!(percents, [18]);
}

#[test]
fn over_a_hundred_is_a_number_not_an_error() {
    assert_eq!(
        read(r#"{"limits": [{"kind": "session", "percent": 104}]}"#).windows[0].percent,
        104
    );
}

#[test]
fn unusable_answers_fail_with_a_readable_problem() {
    let html = read("<html>502 bad gateway</html>");
    assert!(!html.ok());
    assert!(html.windows.is_empty());
    assert!(html.problem.unwrap().contains("json"));
    assert!(!read(r#"{"spend": {}}"#).ok());
    assert!(!read(r#"{"limits": []}"#).ok());
}

#[test]
fn enabled_credits_say_how_much_of_how_much() {
    let reading = read(
        r#"{"limits": [{"kind": "session", "percent": 3}],
        "spend": {"enabled": true,
                  "used": {"amount_minor": 1234, "currency": "BRL", "exponent": 2},
                  "limit": {"amount_minor": 50000, "currency": "BRL", "exponent": 2}}}"#,
    );
    assert_eq!(
        reading.credits.as_deref(),
        Some("Créditos extras: BRL 12,34 de BRL 500,00")
    );
}

#[test]
fn enabled_credits_without_a_ceiling_say_only_how_much() {
    let reading = read(
        r#"{"limits": [{"kind": "session", "percent": 3}],
        "spend": {"enabled": true, "limit": null,
                  "used": {"amount_minor": 1234, "currency": "BRL", "exponent": 2}}}"#,
    );
    assert_eq!(
        reading.credits.as_deref(),
        Some("Créditos extras: BRL 12,34")
    );
}

#[test]
fn a_currency_without_cents_gets_no_cents() {
    let reading = read(
        r#"{"limits": [{"kind": "session", "percent": 3}],
        "spend": {"enabled": true, "limit": null,
                  "used": {"amount_minor": 900, "currency": "JPY", "exponent": 0}}}"#,
    );
    assert_eq!(reading.credits.as_deref(), Some("Créditos extras: JPY 900"));
}

#[test]
fn short_until_uses_the_short_ago_units() {
    let now = Local.with_ymd_and_hms(2026, 9, 10, 13, 0, 0).unwrap();
    let after = |d: TimeDelta| short_until(&(now + d), &now);
    assert_eq!(after(TimeDelta::minutes(40)), "40min");
    assert_eq!(after(TimeDelta::hours(2)), "2h");
    assert_eq!(after(TimeDelta::days(3)), "3d");
    assert_eq!(after(TimeDelta::seconds(40)), "<1min");
    assert_eq!(after(TimeDelta::minutes(-5)), "agora");
    let later = Local.with_ymd_and_hms(2026, 9, 30, 13, 0, 0).unwrap();
    assert_eq!(short_until(&later, &now), "30/9");
}

#[test]
fn token_and_account_come_from_the_given_claude_dirs() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(access_token(dir.path()), None);
    std::fs::write(
        dir.path().join(".credentials.json"),
        r#"{"claudeAiOauth":{"accessToken":""}}"#,
    )
    .unwrap();
    assert_eq!(access_token(dir.path()), None);
    std::fs::write(
        dir.path().join(".credentials.json"),
        r#"{"claudeAiOauth":{"accessToken":"tok"}}"#,
    )
    .unwrap();
    assert_eq!(access_token(dir.path()).as_deref(), Some("tok"));

    let claude_json = dir.path().join(".claude.json");
    assert!(account(&claude_json).is_empty());
    std::fs::write(&claude_json, r#"{"oauthAccount":{"emailAddress":"a@b.c","organizationName":"Marrow","organizationType":"claude_team"}}"#).unwrap();
    let who = account(&claude_json);
    assert_eq!(who.email.as_deref(), Some("a@b.c"));
    assert_eq!(who.headline().as_deref(), Some("Marrow · Claude team"));
    assert_eq!(UsageAccount::default().headline(), None);
}

async fn fake_api(status: StatusCode, body: &'static str) -> String {
    let app = Router::new().route(
        "/api/oauth/usage",
        get(move |headers: HeaderMap| async move {
            if headers["authorization"] != "Bearer tok" {
                return (StatusCode::BAD_REQUEST, "");
            }
            (status, body)
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await });
    format!("http://127.0.0.1:{port}/api/oauth/usage")
}

#[tokio::test]
async fn fetch_reads_with_the_bearer_token_and_maps_failures() {
    let ok = fake_api(
        StatusCode::OK,
        r#"{"limits":[{"kind":"session","percent":3}]}"#,
    )
    .await;
    let reading = fetch_usage(&ok, Some("tok".into()), UsageAccount::default).await;
    assert_eq!(reading.windows[0].percent, 3);

    let expired = fake_api(StatusCode::UNAUTHORIZED, "").await;
    let reading = fetch_usage(&expired, Some("tok".into()), UsageAccount::default).await;
    assert_eq!(reading.problem.as_deref(), Some(EXPIRED));

    let broken = fake_api(StatusCode::BAD_GATEWAY, "").await;
    let reading = fetch_usage(&broken, Some("tok".into()), UsageAccount::default).await;
    assert_eq!(reading.problem.as_deref(), Some("A api respondeu 502"));

    let reading = fetch_usage(&ok, None, UsageAccount::default).await;
    assert_eq!(reading.problem.as_deref(), Some(NO_TOKEN));

    let closed = "http://127.0.0.1:1/api/oauth/usage";
    let reading = fetch_usage(closed, Some("tok".into()), UsageAccount::default).await;
    assert_eq!(reading.problem.as_deref(), Some(OFFLINE));
}
