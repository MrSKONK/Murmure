//! Mêmes cas que la version Python (`tests/cleanup_cases.json`, à la racine du dépôt).
use murmure_core::cleanup::{clean, fit_to_context, Options};
use serde_json::Value;

#[test]
fn cleanup_cases_shared_with_python() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/cleanup_cases.json");
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(cases.len() > 40);
    let mut failures = Vec::new();
    for case in &cases {
        let text = case["text"].as_str().unwrap();
        let expected = case["expected"].as_str().unwrap();
        let got = match case["fn"].as_str().unwrap() {
            "clean" => {
                let mut o = Options::default();
                for (k, v) in case["opts"].as_object().unwrap() {
                    let v = v.as_bool().unwrap();
                    match k.as_str() {
                        "fillers" => o.fillers = v,
                        "commands" => o.commands = v,
                        "french" => o.french = v,
                        "nbsp" => o.nbsp = v,
                        "addresses" => o.addresses = v,
                        "short_no_period" => o.short_no_period = v,
                        other => panic!("option inconnue {other}"),
                    }
                }
                clean(text, o)
            }
            _ => {
                let keep: Vec<String> =
                    case["keep_case"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
                fit_to_context(text, case["before"].as_str(), &keep)
            }
        };
        if got != expected {
            failures.push(format!("{case}\n   obtenu : {got:?}"));
        }
    }
    assert!(failures.is_empty(), "{} cas en échec :\n{}", failures.len(), failures.join("\n"));
}
