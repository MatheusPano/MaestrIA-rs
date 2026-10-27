use std::sync::Arc;

use mx_theme::{
    FACES, HACK, PTY_FALLBACK, StyleCache, Typography, face_by_family, installed_faces,
};
use serde_json::json;

fn none(_: &str) -> bool {
    false
}

fn typo(size: Option<f64>, line: Option<f64>) -> Typography {
    Typography::new(None, size, line, &none)
}

#[test]
fn the_standard_is_hack_13_on_1_2() {
    let t = Typography::standard();
    assert_eq!((t.mono(), t.size(), t.line()), ("Hack", 13.0, 1.2));
    assert_eq!(Typography::default(), t);
    assert_eq!(t.to_string(), "Hack 13.0px/1.2");
}

#[test]
fn a_hand_edited_config_is_clamped_not_obeyed() {
    let wild = Typography::from_json(Some(&json!({"size": 400.0, "line": -3.0})), &none);
    assert_eq!(wild.size(), Typography::MAX_SIZE);
    assert_eq!(wild.line(), Typography::MIN_LINE);
    let wild = Typography::from_json(Some(&json!({"size": 400, "line": 0.2})), &none);
    assert_eq!((wild.size(), wild.line()), (24.0, 1.0));
}

#[test]
fn a_face_this_machine_lacks_falls_back_to_the_bundled_one() {
    assert_eq!(
        Typography::new(Some("Comic Mono do amigo"), None, None, &|_| true).mono(),
        "Hack"
    );
    assert_eq!(
        Typography::new(Some("Menlo"), None, None, &none).mono(),
        "Hack"
    );
    assert_eq!(face_by_family(Some("nada disso"), &none), HACK);
    assert!(installed_faces(&none).contains(&HACK));
}

#[test]
fn an_installed_offered_face_is_kept() {
    let installed = |f: &str| f == "JetBrains Mono";
    assert_eq!(
        Typography::new(Some("JetBrains Mono"), None, None, &installed).mono(),
        "JetBrains Mono"
    );
    assert_eq!(
        installed_faces(&installed)
            .iter()
            .map(|f| f.family)
            .collect::<Vec<_>>(),
        ["Hack", "JetBrains Mono"]
    );
}

#[test]
fn only_what_differs_from_the_standard_is_persisted() {
    assert_eq!(Typography::standard().to_json(), json!({}));
    let bigger = Typography::standard().with(None, Some(16.0), None, &none);
    assert_eq!(bigger.to_json(), json!({"size": 16.0}));
    assert_eq!(
        serde_json::to_string(&bigger.to_json()).unwrap(),
        r#"{"size":16.0}"#
    );
}

#[test]
fn a_non_standard_typography_round_trips_equal() {
    let installed = |f: &str| f == "Iosevka";
    let chosen = Typography::new(Some("Iosevka"), Some(17.0), Some(1.45), &installed);
    let json = chosen.to_json();
    assert_eq!(json, json!({"mono": "Iosevka", "size": 17.0, "line": 1.45}));
    assert_eq!(Typography::from_json(Some(&json), &installed), chosen);
}

#[test]
fn a_missing_or_malformed_type_object_reads_as_standard_or_ignores_bad_fields() {
    assert_eq!(Typography::from_json(None, &none), Typography::standard());
    assert_eq!(
        Typography::from_json(Some(&json!("nada")), &none),
        Typography::standard()
    );
    let t = Typography::from_json(
        Some(&json!({"mono": 3, "size": "grande", "line": 1.5})),
        &none,
    );
    assert_eq!((t.mono(), t.size(), t.line()), ("Hack", 13.0, 1.5));
}

#[test]
fn stepping_the_line_from_1_0_three_times_reads_exactly_1_3() {
    let mut t = typo(None, Some(1.0));
    for _ in 0..3 {
        t = t.with(None, None, Some(t.line() + Typography::LINE_STEP), &none);
    }
    assert_eq!(t.line(), 1.3);
}

#[test]
fn stepping_the_line_from_the_standard_three_times_reads_exactly_1_5() {
    let mut t = Typography::standard();
    for _ in 0..3 {
        t = t.with(None, None, Some(t.line() + Typography::LINE_STEP), &none);
    }
    assert_eq!(t.line(), 1.5);
    assert_eq!(t.to_json()["line"], json!(1.5));
}

#[test]
fn zoom_is_contained_by_what_the_base_allows() {
    let big = typo(Some(Typography::MAX_SIZE), None);
    assert_eq!(big.clamp_zoom(4), 0);
    assert_eq!(big.size_at(4), Typography::MAX_SIZE);
    let small = typo(Some(Typography::MIN_SIZE), None);
    assert_eq!(small.clamp_zoom(-4), 0);
    assert_eq!(small.size_at(2), Typography::MIN_SIZE + 2.0);
}

#[test]
fn twenty_zoom_steps_past_the_ceiling_are_undone_by_one_step_back() {
    let t = Typography::standard();
    let mut zoom = 0;
    for _ in 0..20 {
        zoom = t.clamp_zoom(zoom + 1);
    }
    assert_eq!(t.size_at(zoom), 24.0);
    zoom = t.clamp_zoom(zoom - 1);
    assert_eq!(t.size_at(zoom), 23.0);
}

#[test]
fn changing_the_base_keeps_each_panes_delta() {
    let zoom = Typography::standard().clamp_zoom(2);
    assert_eq!(Typography::standard().size_at(zoom), 15.0);
    assert_eq!(typo(Some(10.0), None).size_at(zoom), 12.0);
}

#[test]
fn a_persisted_zoom_is_clamped_by_the_base_in_force_on_restore() {
    assert_eq!(
        typo(Some(Typography::MAX_SIZE - 1.0), None).clamp_zoom(6),
        1
    );
}

#[test]
fn a_fractional_base_clamps_zoom_to_whole_steps_inside_the_range() {
    let t = typo(Some(13.5), None);
    assert_eq!(t.clamp_zoom(-100), -4);
    assert_eq!(t.clamp_zoom(100), 10);
    assert_eq!(t.size_at(100), 23.5);
}

#[test]
fn sizes_are_rounded_to_two_decimals_on_input() {
    let t = typo(Some(13.456), Some(1.234));
    assert_eq!((t.size(), t.line()), (13.46, 1.23));
}

#[test]
fn the_same_face_size_and_line_give_the_same_style_instance() {
    let t = Typography::standard();
    let mut cache = StyleCache::new();
    let mut made = 0;
    let mut make = |k: &mx_theme::StyleKey| {
        made += 1;
        (k.mono.clone(), k.size(), k.line())
    };
    let first = cache.get_or_insert_with(t.style_key(0), &mut make);
    let again = cache.get_or_insert_with(t.style_key(0), &mut make);
    let zoomed = cache.get_or_insert_with(t.style_key(1), &mut make);
    assert!(Arc::ptr_eq(&first, &again));
    assert!(!Arc::ptr_eq(&first, &zoomed));
    assert_eq!(*zoomed, ("Hack".to_owned(), 14.0, 1.2));
    assert_eq!(made, 2);
    assert_eq!(cache.len(), 2);
}

#[test]
fn the_offered_faces_start_with_bundled_hack_and_the_fallback_chain_is_fixed() {
    assert_eq!(FACES.len(), 31);
    assert_eq!(FACES[0], HACK);
    assert_eq!(
        FACES
            .iter()
            .filter(|f| f.bundled)
            .map(|f| f.family)
            .collect::<Vec<_>>(),
        ["Hack"]
    );
    assert_eq!(FACES[1].note, Some("com os glifos do prompt"));
    assert_eq!(FACES[30].family, "Courier New");
    assert_eq!(PTY_FALLBACK.first(), Some(&"Menlo"));
    assert_eq!(PTY_FALLBACK.last(), Some(&"sans-serif"));
}
