//! The scene list and parameter specs as JSON, so the page builds its
//! controls from the same table the solver reads.

use physicall_core::scene::{ParamSpec, Scale, SceneKind};

pub fn json() -> String {
    let scenes: Vec<String> = SceneKind::ALL
        .iter()
        .map(|k| {
            let params: Vec<String> = k.params().iter().map(param).collect();
            format!(
                r#"{{"id":{},"label":{},"params":[{}]}}"#,
                quote(k.id()),
                quote(k.label()),
                params.join(",")
            )
        })
        .collect();
    format!("[{}]", scenes.join(","))
}

fn param(p: &ParamSpec) -> String {
    let (log, step) = match p.scale {
        Scale::Log => (true, 0.0),
        Scale::Linear { step } => (false, step),
    };
    format!(
        r#"{{"key":{},"label":{},"unit":{},"min":{},"max":{},"default":{},"step":{},"log":{},"rebuild":{}}}"#,
        quote(p.key),
        quote(p.label),
        quote(p.unit),
        p.min,
        p.max,
        p.default,
        step,
        log,
        p.rebuild
    )
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_every_scene_with_plain_decimal_numbers() {
        let json = json();
        for kind in SceneKind::ALL {
            assert!(json.contains(&format!(r#""id":"{}""#, kind.id())));
        }
        assert!(json.contains(r#""min":0.000000001"#), "{json}");
        assert!(
            !json.contains("e-"),
            "JSON numbers must not use exponents: {json}"
        );
    }
}
