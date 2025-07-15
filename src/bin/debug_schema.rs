use serde_json::{json, Value};

fn main() {
    println!("🧪 Testing build_complete_example_object with User schema...");

    // This is the exact User schema from our OpenAPI spec
    let user_schema = json!({
        "type": "object",
        "required": ["id", "name", "email"],
        "properties": {
            "id": {
                "type": "string",
                "format": "uuid",
                "example": "abc-123"
            },
            "name": {
                "type": "string",
                "minLength": 1,
                "maxLength": 100,
                "example": "John"
            },
            "email": {
                "type": "string",
                "format": "email",
                "example": "john@example.com"
            },
            "phone": {
                "type": "string",
                "pattern": "^\\+?[1-9]\\d{1,14}$",
                "example": "+1-555-123-4567"
            }
        }
    });

    println!(
        "📋 Input schema: {}",
        serde_json::to_string_pretty(&user_schema).unwrap()
    );

    // Call our function
    let result = brrtrouter::generator::schema::build_complete_example_object(&user_schema);

    println!(
        "✅ Generated object: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // Check if email field is present
    if let Some(email) = result.get("email") {
        println!("✅ SUCCESS: Email field found: {}", email);
    } else {
        println!("❌ FAIL: Email field missing!");
        println!(
            "Present fields: {:?}",
            result.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
    }
}
