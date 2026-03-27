use askama::Template;
#[derive(Template)]
#[template(source = "{% if let Some(service) = downstream_service %}URL: {{ service }}{% else %}MISSING{% endif %}", ext = "txt")]
struct TestTmpl { downstream_service: Option<String> }
#[test]
fn test_askama() {
    let t = TestTmpl { downstream_service: Some("consignments".to_string()) };
    println!("TEMPLATE EVAL: {}", t.render().unwrap());
}
