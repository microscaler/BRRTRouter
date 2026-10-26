//! `brrtrouter::contract`: a hand-built value is checked against an operation's response schema
//! the way the server's response validation checks it.

use brrtrouter::contract::{ContractError, ResponseContract};
use serde_json::json;

fn pet_store() -> ResponseContract {
    ResponseContract::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/openapi.yaml"
    ))
    .expect("pet store spec")
}

#[test]
fn valid_body_passes() {
    pet_store().assert_valid(
        "get_pet",
        200,
        &json!({"id": 1, "name": "Max", "breed": "Poodle", "age": 3, "vaccinated": true, "tags": []}),
    );
}

#[test]
fn mismatch_lists_each_error_with_its_path() {
    let err = pet_store()
        .check(
            "get_pet",
            200,
            &json!({"id": "1", "name": "Max", "breed": "Poodle", "age": 3, "tags": [1]}),
        )
        .unwrap_err();
    let ContractError::Mismatch { errors, .. } = &err else {
        panic!("expected a mismatch, got {err}");
    };
    let all = errors.join("\n");
    assert!(
        all.contains("vaccinated"),
        "missing required field reported: {all}"
    );
    assert!(
        all.contains("/id"),
        "wrong type reported at its path: {all}"
    );
    assert!(
        all.contains("/tags/0"),
        "array item reported at its path: {all}"
    );
}

#[test]
fn unknown_operation_and_missing_schema_are_errors() {
    let c = pet_store();
    assert_eq!(
        c.check("no_such_op", 200, &json!({})),
        Err(ContractError::UnknownOperation("no_such_op".into()))
    );
    // get_pet's 404 is `application/problem+json`: a JSON-ish schema the server checks too.
    assert!(matches!(
        c.check("get_pet", 404, &json!({"title": 1})),
        Err(ContractError::Mismatch { status: 404, .. })
    ));
    assert!(matches!(
        c.check("get_pet", 418, &json!({})),
        Err(ContractError::NoSchema { status: 418, .. })
    ));
    assert!(c.operations().contains(&"get_pet"));
}

#[test]
#[should_panic(expected = "`get_pet` 200 response does not match its schema")]
fn assert_valid_panics_with_the_errors() {
    pet_store().assert_valid("get_pet", 200, &json!({}));
}

#[test]
fn undeclared_fields_lists_keys_the_schema_does_not_name() {
    let c = pet_store();
    let body = json!({
        "id": 1, "name": "Max", "breed": "Poodle", "age": 3, "vaccinated": true, "tags": [],
        "colour": "brown", "owner": {"x": 1}
    });
    // Valid by type: the Pet schema does not forbid extra keys.
    c.assert_valid("get_pet", 200, &body);
    assert_eq!(
        c.undeclared_fields("get_pet", 200, &body).unwrap(),
        vec!["/colour".to_string(), "/owner".to_string()]
    );
    let clean = json!({"id": 1, "name": "Max", "breed": "Poodle", "age": 3, "vaccinated": true, "tags": []});
    c.assert_strict("get_pet", 200, &clean);
}

#[test]
#[should_panic(expected = "sends fields its schema does not declare: /colour")]
fn assert_strict_panics_on_an_undeclared_field() {
    pet_store().assert_strict(
        "get_pet",
        200,
        &json!({"id": 1, "name": "Max", "breed": "Poodle", "age": 3, "vaccinated": true, "tags": [], "colour": "brown"}),
    );
}
