use std::sync::atomic::{AtomicUsize, Ordering};

pub type Rgb = (u8, u8, u8);

pub struct Palette {
    pub base: Rgb,
    pub mantle: Rgb,
    pub crust: Rgb,
    pub surface0: Rgb,
    pub surface1: Rgb,
    pub surface2: Rgb,
    pub overlay0: Rgb,
    pub text: Rgb,
    pub subtext0: Rgb,
    pub subtext1: Rgb,
    pub lavender: Rgb,
    pub maroon: Rgb,
    pub mauve: Rgb,
    pub peach: Rgb,
    pub yellow: Rgb,
    pub pink: Rgb,
    pub rosewater: Rgb,
}

pub const THEMES: [(&str, &str, Palette); 6] = [
    (
        "mocha",
        "Mocha",
        Palette {
            base: (30, 30, 46),
            mantle: (24, 24, 37),
            crust: (17, 17, 27),
            surface0: (49, 50, 68),
            surface1: (69, 71, 90),
            surface2: (88, 91, 112),
            overlay0: (108, 112, 134),
            text: (205, 214, 244),
            subtext0: (166, 173, 200),
            subtext1: (186, 194, 222),
            lavender: (180, 190, 254),
            maroon: (235, 160, 172),
            mauve: (203, 166, 247),
            peach: (250, 179, 135),
            yellow: (249, 226, 175),
            pink: (245, 194, 231),
            rosewater: (245, 224, 220),
        },
    ),
    (
        "macchiato",
        "Macchiato",
        Palette {
            base: (36, 39, 58),
            mantle: (30, 32, 48),
            crust: (24, 25, 38),
            surface0: (54, 58, 79),
            surface1: (73, 77, 100),
            surface2: (91, 96, 120),
            overlay0: (110, 115, 141),
            text: (202, 211, 245),
            subtext0: (165, 173, 203),
            subtext1: (184, 192, 224),
            lavender: (183, 189, 248),
            maroon: (237, 135, 150),
            mauve: (198, 160, 246),
            peach: (245, 169, 127),
            yellow: (238, 212, 159),
            pink: (245, 189, 230),
            rosewater: (244, 219, 214),
        },
    ),
    (
        "frappe",
        "Frappe",
        Palette {
            base: (48, 52, 70),
            mantle: (41, 44, 60),
            crust: (35, 38, 52),
            surface0: (65, 69, 89),
            surface1: (81, 87, 109),
            surface2: (98, 104, 128),
            overlay0: (115, 121, 148),
            text: (198, 208, 245),
            subtext0: (165, 173, 206),
            subtext1: (181, 191, 226),
            lavender: (186, 187, 241),
            maroon: (234, 153, 156),
            mauve: (202, 158, 230),
            peach: (239, 159, 118),
            yellow: (229, 200, 144),
            pink: (244, 184, 228),
            rosewater: (242, 213, 207),
        },
    ),
    (
        "latte",
        "Latte",
        Palette {
            base: (239, 241, 245),
            mantle: (230, 233, 239),
            crust: (220, 224, 232),
            surface0: (204, 208, 218),
            surface1: (188, 192, 204),
            surface2: (172, 176, 190),
            overlay0: (140, 143, 161),
            text: (76, 79, 105),
            subtext0: (108, 111, 133),
            subtext1: (92, 95, 119),
            lavender: (114, 135, 253),
            maroon: (230, 69, 83),
            mauve: (136, 57, 239),
            peach: (254, 100, 11),
            yellow: (223, 142, 29),
            pink: (234, 118, 203),
            rosewater: (220, 138, 120),
        },
    ),
    (
        "dracula",
        "Dracula",
        Palette {
            base: (40, 42, 54),
            mantle: (33, 34, 44),
            crust: (25, 26, 33),
            surface0: (68, 71, 90),
            surface1: (86, 89, 113),
            surface2: (98, 114, 164),
            overlay0: (110, 113, 145),
            text: (248, 248, 242),
            subtext0: (189, 195, 208),
            subtext1: (208, 212, 225),
            lavender: (189, 147, 249),
            maroon: (255, 85, 85),
            mauve: (189, 147, 249),
            peach: (255, 184, 108),
            yellow: (241, 250, 140),
            pink: (255, 121, 198),
            rosewater: (255, 208, 232),
        },
    ),
    (
        "nord",
        "Nord",
        Palette {
            base: (46, 52, 64),
            mantle: (39, 44, 54),
            crust: (33, 37, 45),
            surface0: (59, 66, 82),
            surface1: (67, 76, 94),
            surface2: (76, 86, 106),
            overlay0: (97, 110, 136),
            text: (236, 239, 244),
            subtext0: (199, 206, 217),
            subtext1: (216, 222, 233),
            lavender: (180, 142, 173),
            maroon: (191, 97, 106),
            mauve: (180, 142, 173),
            peach: (208, 135, 112),
            yellow: (235, 203, 139),
            pink: (180, 142, 173),
            rosewater: (216, 222, 233),
        },
    ),
];

static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub fn set(name: &str) -> &'static str {
    let index = THEMES
        .iter()
        .position(|(key, _, _)| *key == name)
        .unwrap_or(0);
    CURRENT.store(index, Ordering::Relaxed);
    THEMES[index].0
}

pub fn label() -> &'static str {
    let index = CURRENT.load(Ordering::Relaxed);
    THEMES[index].1
}

pub fn next() -> &'static str {
    let index = (CURRENT.load(Ordering::Relaxed) + 1) % THEMES.len();
    set(THEMES[index].0)
}

pub fn palette() -> &'static Palette {
    &THEMES[CURRENT.load(Ordering::Relaxed)].2
}

pub fn is_light() -> bool {
    let (r, g, b) = palette().base;
    (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000 > 150
}

pub fn selected_fg() -> Rgb {
    if is_light() {
        palette().text
    } else {
        palette().rosewater
    }
}

pub fn shade(color: Rgb) -> Rgb {
    if !is_light() {
        return color;
    }
    let (r, g, b) = palette().crust;
    (
        ((color.0 as u16 * 45 + r as u16 * 55) / 100) as u8,
        ((color.1 as u16 * 45 + g as u16 * 55) / 100) as u8,
        ((color.2 as u16 * 45 + b as u16 * 55) / 100) as u8,
    )
}

pub fn base() -> Rgb {
    palette().base
}
pub fn mantle() -> Rgb {
    palette().mantle
}
pub fn crust() -> Rgb {
    palette().crust
}
pub fn surface0() -> Rgb {
    palette().surface0
}
pub fn surface1() -> Rgb {
    palette().surface1
}
pub fn surface2() -> Rgb {
    palette().surface2
}
pub fn overlay0() -> Rgb {
    palette().overlay0
}
pub fn txt() -> Rgb {
    palette().text
}
pub fn sub0() -> Rgb {
    palette().subtext0
}
pub fn sub1() -> Rgb {
    palette().subtext1
}
pub fn lavender() -> Rgb {
    palette().lavender
}
pub fn maroon() -> Rgb {
    palette().maroon
}
pub fn mauve() -> Rgb {
    palette().mauve
}
pub fn peach() -> Rgb {
    palette().peach
}
pub fn yellow() -> Rgb {
    palette().yellow
}
pub fn pink() -> Rgb {
    palette().pink
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_lookup_falls_back_to_mocha() {
        assert_eq!(set("nope"), "mocha");
        assert_eq!(set("latte"), "latte");
        assert_eq!(label(), "Latte");
    }

    #[test]
    fn every_theme_has_unique_key() {
        for (index, (key, _, _)) in THEMES.iter().enumerate() {
            assert!(
                THEMES
                    .iter()
                    .skip(index + 1)
                    .all(|(other, _, _)| other != key),
                "duplicate theme {key}"
            );
        }
    }
}
