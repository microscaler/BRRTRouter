#![allow(clippy::unwrap_used, clippy::expect_used)]
//! OpenAPI `integer` honours `format`, and values above `i32::MAX` survive the
//! request path for `int64` fields.
//!
//! Before this, every `integer` generated `i32` whatever its `format`, so an
//! `int64` field rejected any value above `i32::MAX` (epoch milliseconds, byte
//! counts, epoch seconds from 2038).

use brrtrouter::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
use brrtrouter::generator::{
    collect_component_schemas, extract_fields, integer_type, parameter_to_field, schema_to_type,
};
use brrtrouter::router::ParamVec;
use brrtrouter::server::param_validation::validate_route_parameters;
use brrtrouter::server::request::decode_param_value;
use brrtrouter::spec::{ParameterLocation, ParameterMeta};
use brrtrouter::typed::{TypedHandlerFor, TypedHandlerRequest};
use brrtrouter::validator_cache::ValidatorCache;
use http::Method;
use may::sync::mpsc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use smallvec::smallvec;
use std::io::Write;
use std::sync::Arc;

/// 2100-01-01T00:00:00Z in epoch milliseconds: far above `i32::MAX`.
const EPOCH_MS: i64 = 4_102_444_800_000;

#[test]
fn integer_type_follows_format() {
    let cases = [
        (json!({"type": "integer"}), "i32"),
        (json!({"type": "integer", "format": "int32"}), "i32"),
        (json!({"type": "integer", "format": "int64"}), "i64"),
        (json!({"type": "integer", "format": "uint32"}), "u32"),
        (json!({"type": "integer", "format": "uint64"}), "u64"),
        // Narrower formats fit in i32; unknown formats keep the default.
        (json!({"type": "integer", "format": "int16"}), "i32"),
        (json!({"type": "integer", "format": "epoch"}), "i32"),
        // `minimum: 0` alone does not make a field unsigned.
        (json!({"type": "integer", "minimum": 0}), "i32"),
        (
            json!({"type": "integer", "minimum": 0, "format": "int64"}),
            "i64",
        ),
    ];
    for (schema, want) in cases {
        assert_eq!(integer_type(&schema), want, "{schema}");
        assert_eq!(schema_to_type(&schema), want, "{schema}");
    }
}

#[test]
fn schema_to_type_honours_format_in_arrays_and_nullable() {
    assert_eq!(
        schema_to_type(&json!({"type": "array", "items": {"type": "integer", "format": "int64"}})),
        "Vec<i64>"
    );
    assert_eq!(
        schema_to_type(&json!({"type": "array", "items": {"type": "integer", "format": "uint64"}})),
        "Vec<u64>"
    );
    assert_eq!(
        schema_to_type(&json!({"type": ["integer", "null"], "format": "int64"})),
        "i64"
    );
}

#[test]
fn struct_fields_honour_format() {
    let schema = json!({
        "type": "object",
        "required": ["exp", "count", "bytes", "seq", "plain", "series"],
        "properties": {
            "exp": {"type": "integer", "format": "int64"},
            "count": {"type": "integer", "format": "int32"},
            "bytes": {"type": "integer", "format": "uint64"},
            "seq": {"type": "integer", "format": "uint32"},
            "plain": {"type": "integer"},
            "maybe": {"type": ["integer", "null"], "format": "int64"},
            "series": {"type": "array", "items": {"type": "integer", "format": "int64"}}
        }
    });
    let fields = extract_fields(&schema);
    let ty = |name: &str| {
        fields
            .iter()
            .find(|f| f.original_name == name)
            .map(|f| (f.ty.clone(), f.optional))
            .unwrap()
    };
    assert_eq!(ty("exp"), ("i64".to_string(), false));
    assert_eq!(ty("count"), ("i32".to_string(), false));
    assert_eq!(ty("bytes"), ("u64".to_string(), false));
    assert_eq!(ty("seq"), ("u32".to_string(), false));
    assert_eq!(ty("plain"), ("i32".to_string(), false));
    assert_eq!(ty("maybe"), ("i64".to_string(), true));
    assert_eq!(ty("series"), ("Vec<i64>".to_string(), false));
}

#[test]
fn parameters_honour_format() {
    for (loc, format, want) in [
        (ParameterLocation::Path, "int64", "i64"),
        (ParameterLocation::Query, "int64", "i64"),
        (ParameterLocation::Header, "int64", "i64"),
        (ParameterLocation::Query, "int32", "i32"),
        (ParameterLocation::Query, "uint64", "u64"),
    ] {
        let param = ParameterMeta {
            name: "since".to_string(),
            location: loc.clone(),
            required: true,
            schema: Some(json!({"type": "integer", "format": format})),
            style: None,
            explode: None,
        };
        let field = parameter_to_field(&param);
        assert_eq!(field.ty, want, "{loc:?} {format}");
        assert_eq!(field.value, "42");
    }
}

/// Snapshot of the generated field types for every integer format, end to end
/// from a spec file through `collect_component_schemas`.
#[test]
fn component_schema_snapshot_per_format() {
    let spec = r#"
openapi: 3.1.0
info: {title: integer formats, version: "1.0"}
paths: {}
components:
  schemas:
    Sizes:
      type: object
      required: [plain, i32f, i64f, u32f, u64f, list64]
      properties:
        plain: {type: integer}
        i32f: {type: integer, format: int32}
        i64f: {type: integer, format: int64}
        u32f: {type: integer, format: uint32}
        u64f: {type: integer, format: uint64}
        list64:
          type: array
          items: {type: integer, format: int64}
        nullable64:
          type: [integer, "null"]
          format: int64
"#;
    let mut file = tempfile::Builder::new().suffix(".yaml").tempfile().unwrap();
    file.write_all(spec.as_bytes()).unwrap();
    let types = collect_component_schemas(file.path()).unwrap();
    let sizes = types.get("Sizes").expect("Sizes generated");
    let mut got: Vec<String> = sizes
        .fields
        .iter()
        .map(|f| {
            if f.optional {
                format!("{}: Option<{}>", f.original_name, f.ty)
            } else {
                format!("{}: {}", f.original_name, f.ty)
            }
        })
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            "i32f: i32",
            "i64f: i64",
            "list64: Vec<i64>",
            "nullable64: Option<i64>",
            "plain: i32",
            "u32f: u32",
            "u64f: u64",
        ]
    );
}

#[test]
fn param_validation_rejects_values_outside_the_format() {
    let check = |format: Option<&str>, value: &str| {
        let mut schema = json!({"type": "integer"});
        if let Some(f) = format {
            schema["format"] = json!(f);
        }
        let params = [ParameterMeta {
            name: "n".to_string(),
            location: ParameterLocation::Query,
            required: true,
            schema: Some(schema),
            style: None,
            explode: None,
        }];
        let query: ParamVec = smallvec![(Arc::from("n"), value.to_string())];
        validate_route_parameters(
            &params,
            &ParamVec::new(),
            &query,
            &HeaderVec::new(),
            &HeaderVec::new(),
        )
        .is_ok()
    };
    assert!(check(Some("int64"), "4102444800000"));
    assert!(!check(Some("int64"), "18446744073709551615"));
    assert!(check(Some("int32"), "2147483647"));
    assert!(!check(Some("int32"), "2147483648"));
    assert!(check(Some("uint32"), "4294967295"));
    assert!(!check(Some("uint32"), "-1"));
    assert!(check(Some("uint64"), "18446744073709551615"));
    assert!(!check(Some("uint64"), "-1"));
    // No format: any JSON integer passes, as before.
    assert!(check(None, "4102444800000"));
    assert!(!check(None, "18446744073709551615"));
}

#[test]
fn decode_param_value_keeps_wide_integers_numeric() {
    let int64 = json!({"type": "integer", "format": "int64"});
    assert_eq!(
        decode_param_value("4102444800000", Some(&int64), None, None),
        json!(EPOCH_MS)
    );
    let uint64 = json!({"type": "integer", "format": "uint64"});
    assert_eq!(
        decode_param_value("18446744073709551615", Some(&uint64), None, None),
        json!(u64::MAX)
    );
    // Only `uint64` falls back to u64; an int64 parameter that large stays a string (rejected).
    assert_eq!(
        decode_param_value("18446744073709551615", Some(&int64), None, None),
        json!("18446744073709551615")
    );
    let list = json!({"type": "array", "items": {"type": "integer", "format": "int64"}});
    assert_eq!(
        decode_param_value("4102444800000,1", Some(&list), None, None),
        json!([EPOCH_MS, 1])
    );
}

/// The request type a generated handler declares for an `int64` path, query and
/// header parameter plus an `int64` body field.
#[derive(Debug, Deserialize, Serialize)]
struct Request {
    since: i64,
    until: Option<i64>,
    #[serde(rename = "x-cursor")]
    cursor: i64,
    ts: i64,
    sizes: Vec<u64>,
}

/// Mirrors the `TryFrom<HandlerRequest>` that `templates/handler.rs.txt` emits.
impl TryFrom<HandlerRequest> for Request {
    type Error = anyhow::Error;

    fn try_from(req: HandlerRequest) -> Result<Self, Self::Error> {
        let int64 = json!({"type": "integer", "format": "int64"});
        let mut data_map = Map::new();
        if let Some(v) = req.get_path_param("since") {
            data_map.insert(
                "since".into(),
                decode_param_value(v, Some(&int64), None, None),
            );
        }
        if let Some(v) = req.get_query_param("until") {
            data_map.insert(
                "until".into(),
                decode_param_value(v, Some(&int64), None, None),
            );
        }
        if let Some(v) = req.get_header("x-cursor") {
            data_map.insert(
                "x-cursor".into(),
                decode_param_value(v, Some(&int64), None, None),
            );
        }
        if let Some(Value::Object(map)) = req.body {
            data_map.extend(map);
        }
        Ok(serde_json::from_value(Value::Object(data_map))?)
    }
}

#[test]
fn int64_request_round_trip_above_i32_max() {
    let body = json!({"ts": EPOCH_MS, "sizes": [u64::MAX, 5_000_000_000u64]});

    // The request validator accepts the value for an `int64` schema.
    let body_schema = json!({
        "type": "object",
        "required": ["ts"],
        "properties": {
            "ts": {"type": "integer", "format": "int64"},
            "sizes": {"type": "array", "items": {"type": "integer", "format": "uint64", "minimum": 0}}
        }
    });
    let validator = ValidatorCache::new(false)
        .get_or_compile("int64_round_trip", "request", None, &body_schema)
        .expect("schema compiles");
    assert!(validator.is_valid(&body));
    // Unsigned formats state `minimum: 0` so the validator rejects negatives before the typed
    // conversion would.
    assert!(!validator.is_valid(&json!({"ts": EPOCH_MS, "sizes": [-1]})));

    let (tx, _rx) = mpsc::channel::<Box<HandlerResponse>>();
    let epoch = EPOCH_MS.to_string();
    let path_params: ParamVec = smallvec![(Arc::from("since"), epoch.clone())];
    let query_params: ParamVec = smallvec![(Arc::from("until"), epoch.clone())];
    let headers: HeaderVec = smallvec![(Arc::from("x-cursor"), epoch.clone())];
    let req = HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::POST,
        path: format!("/events/{epoch}"),
        handler_name: "int64_round_trip".to_string(),
        path_params,
        query_params,
        raw_query: None,
        headers,
        cookies: HeaderVec::new(),
        body: Some(body.clone()),
        jwt_claims: None,
        reply_tx: tx,
        queue_guard: None,
        span: tracing::Span::none(),
    };

    let typed = TypedHandlerRequest::<Request>::from_handler(req).expect("int64 converts");
    assert_eq!(typed.data.since, EPOCH_MS);
    assert_eq!(typed.data.until, Some(EPOCH_MS));
    assert_eq!(typed.data.cursor, EPOCH_MS);
    assert_eq!(typed.data.ts, EPOCH_MS);
    assert_eq!(typed.data.sizes, vec![u64::MAX, 5_000_000_000]);

    // And back out: the response JSON carries the same number.
    let out = serde_json::to_value(&typed.data).unwrap();
    assert_eq!(out["ts"], json!(EPOCH_MS));
    assert_eq!(out["since"], json!(EPOCH_MS));
    assert_eq!(out["sizes"], body["sizes"]);
}
