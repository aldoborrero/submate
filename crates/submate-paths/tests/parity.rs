//! Parity tests against golden fixtures captured.

use parity::{assert_str_eq, golden};
use serde_json::Value;
use submate_paths::{build_subtitle_path, map_path, SubtitleNaming};
use submate_types::LanguageNamingType;

fn naming_type_from(args: &Value) -> LanguageNamingType {
    match args.get("naming_type") {
        None | Some(Value::Null) => LanguageNamingType::Iso6392B,
        Some(Value::String(s)) => match s.as_str() {
            "iso_639_1" => LanguageNamingType::Iso6391,
            "iso_639_2_t" => LanguageNamingType::Iso6392T,
            "iso_639_2_b" => LanguageNamingType::Iso6392B,
            "name" => LanguageNamingType::Name,
            "native" => LanguageNamingType::Native,
            other => panic!("unknown naming_type {other:?}"),
        },
        other => panic!("expected string naming_type, got {other:?}"),
    }
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    match args.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.as_str()),
        other => panic!("expected string for {key}, got {other:?}"),
    }
}

fn bool_arg(args: &Value, key: &str) -> bool {
    match args.get(key) {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        other => panic!("expected bool for {key}, got {other:?}"),
    }
}

#[test]
fn path_cases() {
    let cases = golden("paths/path_cases.json");

    let build = cases["build_subtitle_path"]
        .as_object()
        .expect("build_subtitle_path section");
    for (name, case) in build {
        let args = &case["args"];
        let video_path = str_arg(args, "video_path").expect("video_path");
        let language = str_arg(args, "language");
        let model_name = str_arg(args, "model_name").unwrap_or("");
        let extension = str_arg(args, "extension").unwrap_or(".srt");

        let naming = SubtitleNaming {
            naming_type: naming_type_from(args),
            include_subgen_marker: bool_arg(args, "include_subgen_marker"),
            include_model: bool_arg(args, "include_model"),
            model_name,
            extension,
        };

        let actual = build_subtitle_path(video_path, language, &naming);
        let expected = case["result"].as_str().expect("result string");
        assert_str_eq(&actual, expected);
        println!("build_subtitle_path::{name} ok");
    }

    let map = cases["map_path"].as_object().expect("map_path section");
    for (name, case) in map {
        let args = &case["args"];
        let path = str_arg(args, "path").expect("path");
        let use_mapping = bool_arg(args, "use_mapping");
        let path_from = str_arg(args, "path_from").unwrap_or("");
        let path_to = str_arg(args, "path_to").unwrap_or("");

        let actual = map_path(path, use_mapping, path_from, path_to);
        let expected = case["result"].as_str().expect("result string");
        assert_str_eq(&actual, expected);
        println!("map_path::{name} ok");
    }
}
