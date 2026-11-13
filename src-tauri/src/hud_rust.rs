//! The in-game HUD for Rust (Horizon) instances. Horizon has its own HUD widgets (FPS, CPS, keystrokes, clock,
//! ping, coordinates, armour, effects) and its own in-game HUD editor, all configured in `horizon-extras.txt`.
//! So instead of an agent, the launcher writes the HUD page's choices into that file: which widgets are on,
//! where they sit and how big they are, and the shared style (background on/off and opacity, text colour, shadow).
//! Horizon styles all widgets alike, so per-element colours, borders and rainbow text do not carry over.
use crate::{instances, paths};
use anyhow::Result;
use serde_json::{json, Value};

/// launcher id, Horizon's option key, Horizon's widget name
const WIDGETS: [(&str, &str, &str); 8] = [
    ("fps", "hudFps", "fps"),
    ("cps", "hudCps", "cps"),
    ("keystrokes", "hudKeys", "keys"),
    ("clock", "hudClock", "clock"),
    ("ping", "hudPing", "ping"),
    ("coords", "hudCoords", "coords"),
    ("armor", "hudArmour", "armour"),
    ("potions", "hudEffects", "effects"),
];

/// Horizon's text colours are a fixed list.
const PALETTE: [(&str, u32); 9] = [("white", 0xFFFFFF), ("red", 0xFF0000), ("orange", 0xFFA020), ("yellow", 0xFFFF40), ("green", 0x40FF40), ("cyan", 0x40FFFF), ("blue", 0x4080FF), ("magenta", 0xFF40FF), ("black", 0)];

pub fn nearest_colour(hex: &str) -> &'static str {
    let Some(rgb) = hex.strip_prefix('#').and_then(|h| u32::from_str_radix(h, 16).ok()) else { return "white" };
    let (r, g, b) = ((rgb >> 16) as i32, ((rgb >> 8) & 255) as i32, (rgb & 255) as i32);
    PALETTE.iter().min_by_key(|(_, c)| { let (cr, cg, cb) = ((c >> 16) as i32, ((c >> 8) & 255) as i32, (c & 255) as i32); (r - cr).pow(2) + (g - cg).pow(2) + (b - cb).pow(2) }).map(|(n, _)| *n).unwrap_or("white")
}

/// What the launcher uses before the HUD page has ever been saved (same as the page's defaults).
pub fn default_hud() -> Value {
    let el = |on: bool, x: f64, y: f64| json!({ "enabled": on, "x": x, "y": y, "scale": 1.0, "valueColor": "#ffffff", "bgOpacity": 0.55, "shadow": true });
    json!({ "enabled": true, "scale": 1.0, "elements": {
        "fps": el(true, 0.01, 0.02), "cps": el(true, 0.01, 0.10), "ping": el(true, 0.01, 0.18), "coords": el(true, 0.01, 0.26),
        "keystrokes": el(true, 0.01, 0.72), "armor": el(true, 0.96, 0.74), "clock": el(false, 0.5, 0.01), "potions": el(false, 0.99, 0.02) } })
}

fn f(v: Option<&Value>, d: f64) -> f64 { v.and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(d) }

/// Rewrite `horizon-extras.txt`: keep every line that is not one of ours, then add ours.
pub fn merge_extras(existing: &str, hud: &Value) -> String {
    let all_on = hud.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let gscale = f(hud.get("scale"), 1.0);
    let els = hud.get("elements");
    let get = |id: &str| els.and_then(|e| e.get(id));
    let on = |id: &str| all_on && get(id).and_then(|e| e.get("enabled")).and_then(Value::as_bool).unwrap_or(false);

    let managed_keys: Vec<&str> = WIDGETS.iter().map(|w| w.1).chain(["hudBackground", "hudOpacity", "hudTextColour", "hudShadow"]).collect();
    let mut out: Vec<String> = existing.lines().filter(|l| {
        let (k, v) = l.split_once(':').unwrap_or((l, ""));
        let ours_place = k == "hudPlace" && WIDGETS.iter().any(|w| v.split(',').next() == Some(w.2));
        !(managed_keys.contains(&k) || ours_place)
    }).map(String::from).collect();

    let active: Vec<&str> = WIDGETS.iter().map(|w| w.0).filter(|id| on(id)).collect();
    for (id, key, name) in WIDGETS {
        out.push(format!("{key}:{}", on(id)));
        if on(id) {
            let e = get(id);
            let (x, y) = (f(e.and_then(|e| e.get("x")), 0.0).clamp(0.0, 1.0), f(e.and_then(|e| e.get("y")), 0.0).clamp(0.0, 1.0));
            let pct = (f(e.and_then(|e| e.get("scale")), 1.0) * gscale * 100.0).round().clamp(50.0, 200.0) as u32;
            out.push(format!("hudPlace:{name},{x:.4},{y:.4},{pct}"));
        }
    }
    // shared style, taken from the widgets that are on
    let opacity = active.iter().map(|id| f(get(id).and_then(|e| e.get("bgOpacity")), 0.55)).fold(0.0, f64::max);
    out.push(format!("hudBackground:{}", opacity > 0.0));
    out.push(format!("hudOpacity:{}", (opacity * 100.0).round().clamp(0.0, 100.0) as u32));
    let colour = active.first().and_then(|id| get(id)).and_then(|e| e.get("valueColor")).and_then(Value::as_str).map(nearest_colour).unwrap_or("white");
    out.push(format!("hudTextColour:{colour}"));
    out.push(format!("hudShadow:{}", active.iter().any(|id| get(id).and_then(|e| e.get("shadow")).and_then(Value::as_bool).unwrap_or(true))));
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// Write the settings into one Rust instance.
pub fn apply(id: &str, hud: &Value) -> Result<()> {
    let dir = paths::game_dir(id);
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("horizon-extras.txt");
    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    std::fs::write(file, merge_extras(&existing, hud))?;
    Ok(())
}

/// Write the settings into every Rust instance that has the HUD turned on (best effort).
pub fn apply_all(hud: &Value) {
    if let Ok(list) = instances::list() {
        for i in list.iter().filter(|i| i.loader == "rust" && i.hud) {
            let _ = apply(&i.id, hud);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_map_to_horizons_palette() {
        assert_eq!(nearest_colour("#ffffff"), "white");
        assert_eq!(nearest_colour("#ff3030"), "red");
        assert_eq!(nearest_colour("#3dfcff"), "cyan");
        assert_eq!(nearest_colour("#101010"), "black");
        assert_eq!(nearest_colour("nonsense"), "white");
    }

    #[test]
    fn writes_widgets_style_and_keeps_other_lines() {
        let old = "fullbright:true\nhudFps:false\nhudPlace:fps,0.9000,0.9000,100\nhudPlace:reach,0.1000,0.2000,150\nhudReach:true\nhudEditorKey:key.keyboard.h\n";
        let mut hud = default_hud();
        hud["elements"]["fps"]["x"] = json!(0.25);
        hud["elements"]["fps"]["scale"] = json!(1.5);
        let out = merge_extras(old, &hud);
        for keep in ["fullbright:true", "hudPlace:reach,0.1000,0.2000,150", "hudReach:true", "hudEditorKey:key.keyboard.h"] { assert!(out.contains(keep), "lost {keep}"); }
        assert!(out.contains("hudFps:true") && !out.contains("hudFps:false"));
        assert!(out.contains("hudPlace:fps,0.2500,0.0200,150") && !out.contains("0.9000,0.9000"));
        assert!(out.contains("hudKeys:true") && out.contains("hudPlace:keys,0.0100,0.7200,100"));
        assert!(out.contains("hudClock:false") && !out.contains("hudPlace:clock"));
        assert!(out.contains("hudBackground:true") && out.contains("hudOpacity:55") && out.contains("hudTextColour:white") && out.contains("hudShadow:true"));
    }

    #[test]
    fn transparent_means_no_background_and_master_switch_turns_everything_off() {
        let mut hud = default_hud();
        for id in ["fps", "cps", "ping", "coords", "keystrokes", "armor"] { hud["elements"][id]["bgOpacity"] = json!(0.0); }
        let out = merge_extras("", &hud);
        assert!(out.contains("hudBackground:false") && out.contains("hudOpacity:0"));
        hud["enabled"] = json!(false);
        let off = merge_extras(&out, &hud);
        assert!(off.contains("hudFps:false") && off.contains("hudArmour:false") && !off.contains("hudPlace:"));
        assert_eq!(off.matches("hudFps:").count(), 1); // never duplicated
    }

    #[test]
    fn scale_is_clamped_to_horizons_range() {
        let mut hud = default_hud();
        hud["elements"]["fps"]["scale"] = json!(3.0);
        hud["scale"] = json!(2.0);
        assert!(merge_extras("", &hud).contains("hudPlace:fps,0.0100,0.0200,200"));
        hud["elements"]["fps"]["scale"] = json!(0.5);
        hud["scale"] = json!(0.5);
        assert!(merge_extras("", &hud).contains("hudPlace:fps,0.0100,0.0200,50"));
    }
}
