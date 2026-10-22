//! Country flags, drawn: Windows has no flag emoji and shows the two
//! letters instead (🇨🇭 → "CH"). Also the flags of api0's AI regions.

use dioxus::prelude::*;

use crate::model::Country;

/// A five-pointed star of radius 1 around the origin, for Singapore's flag.
const STAR: &str = "0,-1 0.24,-0.32 0.95,-0.31 0.38,0.12 0.59,0.81 0,0.4 \
                    -0.59,0.81 -0.38,0.12 -0.95,-0.31 -0.24,-0.32";

#[component]
pub fn Flag(country: Country) -> Element {
    // 3:2, cropped to fit where a flag is wider (UK, UAE).
    let flag = match country {
        Country::Switzerland => rsx! {
            rect { width: "30", height: "20", fill: "#DA291C" }
            rect { x: "13", y: "4", width: "4", height: "12", fill: "#fff" }
            rect { x: "9", y: "8", width: "12", height: "4", fill: "#fff" }
        },
        Country::Singapore => rsx! {
            rect { width: "30", height: "10", fill: "#EF3340" }
            rect { y: "10", width: "30", height: "10", fill: "#fff" }
            circle { cx: "6.5", cy: "5", r: "3.5", fill: "#fff" }
            circle { cx: "7.8", cy: "5", r: "3.3", fill: "#EF3340" }
            for (x, y) in [(9.3, 3.5), (10.73, 4.54), (10.18, 6.21), (8.42, 6.21), (7.87, 4.54)] {
                polygon { points: STAR, fill: "#fff", transform: "translate({x} {y}) scale(0.75)" }
            }
        },
        Country::Germany => rsx! {
            rect { width: "30", height: "20", fill: "#000" }
            rect { y: "6.67", width: "30", height: "13.33", fill: "#DD0000" }
            rect { y: "13.33", width: "30", height: "6.67", fill: "#FFCE00" }
        },
        Country::France => rsx! {
            rect { width: "30", height: "20", fill: "#fff" }
            rect { width: "10", height: "20", fill: "#002395" }
            rect { x: "20", width: "10", height: "20", fill: "#ED2939" }
        },
        Country::UnitedKingdom => rsx! {
            defs {
                clipPath { id: "flag-uk-diagonals",
                    path { d: "M30,15 h30 v15 z v15 h-30 z h-30 v-15 z v-15 h30 z" }
                }
            }
            // The usual 60×30 drawing, scaled to the 20 high and centred.
            g { transform: "translate(-5 0) scale(0.6667)",
                rect { width: "60", height: "30", fill: "#012169" }
                path { d: "M0,0 L60,30 M60,0 L0,30", stroke: "#fff", "stroke-width": "6" }
                path {
                    d: "M0,0 L60,30 M60,0 L0,30",
                    stroke: "#C8102E",
                    "stroke-width": "4",
                    "clip-path": "url(#flag-uk-diagonals)",
                }
                path { d: "M30,0 v30 M0,15 h60", stroke: "#fff", "stroke-width": "10" }
                path { d: "M30,0 v30 M0,15 h60", stroke: "#C8102E", "stroke-width": "6" }
            }
        },
        Country::Uae => rsx! {
            rect { width: "30", height: "6.67", fill: "#00732F" }
            rect { y: "6.67", width: "30", height: "6.67", fill: "#fff" }
            rect { y: "13.33", width: "30", height: "6.67", fill: "#000" }
            rect { width: "7.5", height: "20", fill: "#FF0000" }
        },
        Country::Ireland => rsx! {
            rect { width: "30", height: "20", fill: "#fff" }
            rect { width: "10", height: "20", fill: "#169B62" }
            rect { x: "20", width: "10", height: "20", fill: "#FF883E" }
        },
        Country::Belgium => rsx! {
            rect { width: "30", height: "20", fill: "#FDDA24" }
            rect { width: "10", height: "20", fill: "#000" }
            rect { x: "20", width: "10", height: "20", fill: "#EF3340" }
        },
        Country::Austria => rsx! {
            rect { width: "30", height: "20", fill: "#C8102E" }
            rect { y: "6.67", width: "30", height: "6.67", fill: "#fff" }
        },
    };
    rsx! {
        svg { class: "flag", view_box: "0 0 30 20", "aria-hidden": "true", {flag} }
    }
}

/// The flag of one of api0's AI regions, by its id.
#[component]
pub fn RegionFlag(id: &'static str) -> Element {
    let flag = match id {
        "europe" => rsx! {
            rect { width: "30", height: "20", fill: "#003399" }
            // Twelve stars on a circle, as dots: a star this small is a dot.
            for i in 0..12 {
                circle {
                    cx: "{15.0 + 6.0 * (f64::from(i) * std::f64::consts::PI / 6.0).sin()}",
                    cy: "{10.0 - 6.0 * (f64::from(i) * std::f64::consts::PI / 6.0).cos()}",
                    r: "0.9",
                    fill: "#FFCC00",
                }
            }
        },
        "china" => rsx! {
            rect { width: "30", height: "20", fill: "#EE1C25" }
            polygon { points: STAR, fill: "#FFFF00", transform: "translate(5 5) scale(3)" }
            for (x, y) in [(10.0, 2.0), (12.0, 4.0), (12.0, 7.0), (10.0, 9.0)] {
                polygon { points: STAR, fill: "#FFFF00", transform: "translate({x} {y}) scale(1)" }
            }
        },
        "usa" => rsx! {
            rect { width: "30", height: "20", fill: "#fff" }
            for i in 0..7 {
                rect { y: "{f64::from(i) * 40.0 / 13.0}", width: "30", height: "{20.0 / 13.0}", fill: "#B22234" }
            }
            rect { width: "12", height: "{140.0 / 13.0}", fill: "#3C3B6E" }
        },
        _ => rsx! {
            rect { width: "30", height: "20", fill: "#888" }
        },
    };
    rsx! {
        svg { class: "flag", view_box: "0 0 30 20", "aria-hidden": "true", {flag} }
    }
}
