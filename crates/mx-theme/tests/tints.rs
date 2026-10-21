use mx_theme::{Color, Themes, Tint, built_in, built_ins};

const REFERENCE: [(&str, [u32; 13]); 19] = [
    (
        "maestria",
        [
            0xFD968E, 0xF4A065, 0xD0B650, 0xA5C569, 0x79CD8C, 0x49D1B2, 0x2ECDDC, 0x5BC3FB,
            0xB6A9FF, 0xE499DD, 0xF794B3, 0xAA7B5B, 0x5C636F,
        ],
    ),
    (
        "ayu-dark",
        [
            0xFF9890, 0xFFA362, 0xD8BB47, 0xA9CC65, 0x77D58D, 0x3CD9B7, 0x00D5E6, 0x54CAFF,
            0xBCADFF, 0xED9BE6, 0xFF96B9, 0xB27E5A, 0x686868,
        ],
    ),
    (
        "claude-dark",
        [
            0xDD867F, 0xD68E5D, 0xB7A14B, 0x92AD5F, 0x6DB57D, 0x47B89E, 0x34B5C2, 0x55ACDC,
            0xA095E1, 0xC888C2, 0xD9849F, 0x956D51, 0x6E6A61,
        ],
    ),
    (
        "cobalt",
        [
            0xFF9C93, 0xFFAB55, 0xEDC925, 0xB3DD5A, 0x70E990, 0x00EDC5, 0x00E9FE, 0x37DBFF,
            0xCBB7FF, 0xFFA1FE, 0xFF9AC7, 0xC68559, 0x5B87AD,
        ],
    ),
    (
        "everforest-dark",
        [
            0xE79992, 0xE0A075, 0xC4B068, 0xA3BC78, 0x84C290, 0x68C5AD, 0x5CC2CE, 0x71BBE5,
            0xAFA7EA, 0xD49BCE, 0xE397AE, 0x9E7A61, 0x859289,
        ],
    ),
    (
        "high-contrast",
        [
            0xFF7871, 0xFF8A00, 0xD8AD00, 0x96C502, 0x36D26C, 0x00D7AB, 0x00D1EB, 0x00C1FF,
            0xB298FF, 0xF37FEA, 0xFF75AD, 0xB76F39, 0x8A8A8A,
        ],
    ),
    (
        "night-owl",
        [
            0xFF8078, 0xFB8E36, 0xCFAB00, 0x97BF3A, 0x52CA74, 0x00CFA8, 0x00CADF, 0x00BCFF,
            0xAE9AFF, 0xE785DF, 0xFF7DAA, 0xAD7044, 0x5F7E97,
        ],
    ),
    (
        "gruvbox-dark",
        [
            0xDA6A64, 0xD1762D, 0xAC8E00, 0x7D9F31, 0x44A860, 0x00AC8C, 0x00A8BA, 0x009DDA,
            0x9180E1, 0xC06EBA, 0xD4688D, 0x905C38, 0x928374,
        ],
    ),
    (
        "solarized-dark",
        [
            0xE15E59, 0xD76C00, 0xAE8A00, 0x779E00, 0x25A954, 0x00AD88, 0x00A8BD, 0x009AE3,
            0x8F78EA, 0xC464BD, 0xDB5B8A, 0x93572A, 0x586E75,
        ],
    ),
    (
        "graphite",
        [
            0xF3736D, 0xE9812B, 0xBF9D00, 0x8AB030, 0x47BB69, 0x00BF9B, 0x00BACF, 0x00ADF4,
            0xA08CFB, 0xD678CF, 0xED719C, 0xA1663C, 0x6E6E6E,
        ],
    ),
    (
        "synthwave-84",
        [
            0xFF948B, 0xFFA34A, 0xE6C100, 0xABD64E, 0x66E287, 0x00E6BE, 0x00E2F8, 0x20D3FF,
            0xC4AFFF, 0xFF99F8, 0xFF91C0, 0xC18052, 0x848BBD,
        ],
    ),
    (
        "slate",
        [
            0xEB9D96, 0xE4A57A, 0xC8B56D, 0xA7C07C, 0x89C695, 0x6EC9B1, 0x62C7D2, 0x76BFE9,
            0xB4ABEE, 0xD89FD2, 0xE89CB2, 0xA17D65, 0x6B7584,
        ],
    ),
    (
        "monokai-pro",
        [
            0xFF9991, 0xFFA562, 0xDBBD47, 0xAACE66, 0x78D88E, 0x3ADBBA, 0x00D7E9, 0x54CCFF,
            0xBEAEFF, 0xF09DE9, 0xFF97BB, 0xB47F5B, 0x727072,
        ],
    ),
    (
        "nord",
        [
            0xDA9791, 0xD39D79, 0xBBAA6E, 0x9FB47B, 0x85BA8F, 0x70BCA8, 0x67BAC3, 0x76B3D7,
            0xA9A2DB, 0xC998C3, 0xD695A8, 0x957662, 0x4C566A,
        ],
    ),
    (
        "dracula",
        [
            0xFF9B92, 0xFFA956, 0xEAC628, 0xB1DA5A, 0x70E68E, 0x00EAC2, 0x00E5FA, 0x3AD8FF,
            0xC8B4FF, 0xFF9FFA, 0xFF98C4, 0xC38458, 0x6272A4,
        ],
    ),
    (
        "zenburn",
        [
            0xCD958F, 0xC89A7C, 0xB2A573, 0x9BAD7D, 0x86B28E, 0x75B4A2, 0x6FB2BA, 0x7AADCB,
            0xA49ECE, 0xBE96BA, 0xCA93A3, 0x8D7362, 0x709080,
        ],
    ),
    (
        "github-light",
        [
            0xB1403D, 0xA94E00, 0x866800, 0x587900, 0x008239, 0x008566, 0x008193, 0x0075B3,
            0x6C58B9, 0x994693, 0xAB3F68, 0x723F17, 0x57606A,
        ],
    ),
    (
        "rose-pine-dawn",
        [
            0xC0706A, 0xB9784A, 0x9D8939, 0x7B944D, 0x599B68, 0x349E86, 0x1E9BA7, 0x4293BE,
            0x897FC3, 0xAC72A7, 0xBC6E87, 0x805B42, 0x9893A5,
        ],
    ),
    (
        "gruvbox-light",
        [
            0xB04D49, 0xA8580F, 0x896E00, 0x5F7D14, 0x288545, 0x00886C, 0x008594, 0x007AB1,
            0x7161B6, 0x9A5194, 0xAB4C6D, 0x724525, 0x928374,
        ],
    ),
];

fn delta_e(a: Color, b: Color) -> f64 {
    fn lab(c: Color) -> [f64; 3] {
        let lin = |v: f64| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let (r, g, b) = (lin(c.red()), lin(c.green()), lin(c.blue()));
        let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
        let f = |t: f64| {
            if t > 0.008856 {
                t.powf(1.0 / 3.0)
            } else {
                7.787 * t + 16.0 / 116.0
            }
        };
        [
            116.0 * f(y) - 16.0,
            500.0 * (f(x) - f(y)),
            200.0 * (f(y) - f(z)),
        ]
    }
    let (p, q) = (lab(a), lab(b));
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

#[test]
fn there_are_thirteen_labels_in_colour_wheel_order_with_grey_last() {
    let labels: Vec<_> = Tint::ALL.iter().map(|t| t.label()).collect();
    assert_eq!(
        labels,
        [
            "Vermelho",
            "Laranja",
            "Amarelo",
            "Lima",
            "Verde",
            "Verde-água",
            "Ciano",
            "Azul-petróleo",
            "Violeta",
            "Magenta",
            "Rosa",
            "Marrom",
            "Cinza",
        ]
    );
}

#[test]
fn every_built_in_matches_the_reference_table_exactly() {
    assert_eq!(REFERENCE.len(), built_ins().len());
    for (id, expected) in REFERENCE {
        let theme = built_in(id).unwrap();
        for (tint, hex) in Tint::ALL.into_iter().zip(expected) {
            assert_eq!(theme.tint(tint), Color::opaque(hex), "{id} {}", tint.name());
        }
    }
}

#[test]
fn no_two_tints_are_closer_than_delta_e_10_in_any_built_in() {
    for theme in built_ins() {
        for (i, a) in Tint::ALL.into_iter().enumerate() {
            for b in Tint::ALL.into_iter().skip(i + 1) {
                let d = delta_e(theme.tint(a), theme.tint(b));
                assert!(
                    d >= 10.0,
                    "{} e {} no {}: {d}",
                    a.label(),
                    b.label(),
                    theme.id
                );
            }
        }
    }
}

#[test]
fn no_tint_disappears_into_the_pane_background_in_any_built_in() {
    for theme in built_ins() {
        for tint in Tint::ALL {
            let d = delta_e(theme.tint(tint), theme.bg);
            assert!(d >= 15.0, "{} no {}: {d}", tint.label(), theme.id);
        }
    }
}

#[test]
fn the_same_name_gives_another_colour_on_another_palette_and_the_same_on_return() {
    let themes = Themes::new();
    let nord = themes.by_id(Some("nord")).tint(Tint::Cyan);
    let light = themes.by_id(Some("github-light")).tint(Tint::Cyan);
    assert_ne!(nord, light);
    assert_eq!(themes.by_id(Some("nord")).tint(Tint::Cyan), nord);
}

#[test]
fn every_tint_round_trips_through_its_name() {
    for tint in Tint::ALL {
        assert_eq!(Tint::by_name(Some(tint.name())), Some(tint));
    }
}

#[test]
fn an_unknown_or_retired_name_reads_as_no_colour() {
    assert_eq!(Tint::by_name(Some("blue")), None);
    assert_eq!(Tint::by_name(Some("Red")), None);
    assert_eq!(Tint::by_name(None), None);
}

#[test]
fn grey_is_bright_black_falling_back_to_black() {
    let nord = built_in("nord").unwrap();
    assert_eq!(nord.tint(Tint::Grey), nord.ansi.bright_black());
    let mut palette = nord.palette().clone();
    palette.ansi.bright[0] = None;
    assert_eq!(
        mx_theme::Theme::new(palette).tint(Tint::Grey),
        nord.ansi.black
    );
}
