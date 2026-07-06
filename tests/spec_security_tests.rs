#![allow(clippy::unwrap_used)]

use brrtrouter::load_spec;
use std::path::PathBuf;

const FIXTURE: &str = r#"openapi: 3.1.0
info:
  title: Security Inheritance API
  version: "1.0.0"
security:
  - BearerAuth: []
paths:
  /auth/login:
    post:
      operationId: auth_login
      security: []
      responses:
        "200":
          description: OK
  /users/me:
    get:
      operationId: users_me
      security:
        - BearerAuth: []
      responses:
        "200":
          description: OK
  /admin/settings:
    get:
      operationId: admin_settings
      responses:
        "200":
          description: OK
components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
"#;

fn write_temp_yaml(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "brrtr_security_test_{}_{}.yaml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, contents).unwrap();
    path
}

fn route_security(
    routes: &[brrtrouter::spec::RouteMeta],
    handler: &str,
) -> Vec<brrtrouter::spec::SecurityRequirement> {
    routes
        .iter()
        .find(|r| r.handler_name.as_ref() == handler)
        .map(|r| r.security.clone())
        .unwrap_or_else(|| panic!("handler {handler} not found"))
}

#[test]
fn security_empty_array_overrides_global() {
    let path = write_temp_yaml(FIXTURE);
    let (routes, _) = load_spec(path.to_str().unwrap()).unwrap();

    assert!(
        route_security(&routes, "auth_login").is_empty(),
        "explicit security: [] must be public even with global BearerAuth"
    );
}

#[test]
fn security_omitted_inherits_global() {
    let path = write_temp_yaml(FIXTURE);
    let (routes, _) = load_spec(path.to_str().unwrap()).unwrap();

    let inherited = route_security(&routes, "admin_settings");
    assert_eq!(inherited.len(), 1);
    assert!(inherited[0].0.contains_key("BearerAuth"));
}

#[test]
fn security_explicit_requirement_preserved() {
    let path = write_temp_yaml(FIXTURE);
    let (routes, _) = load_spec(path.to_str().unwrap()).unwrap();

    let explicit = route_security(&routes, "users_me");
    assert_eq!(explicit.len(), 1);
    assert!(explicit[0].0.contains_key("BearerAuth"));
}
