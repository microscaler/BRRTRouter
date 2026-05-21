// Minimal standalone JWT verification debug
use base64::Engine as _;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug)]
struct Claims {
    iss: String,
    aud: String,
    exp: i64,
    scope: String,
}

fn base64url_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn base64url_decode(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .unwrap()
}

fn main() {
    let secret = b"test-secret-key-12345";
    let kid = "k1";
    let jwks_k = base64url_encode(secret);

    println!("=== JWT Verification Debug ===");
    println!("Secret bytes: {:?}", secret);
    println!("JWKS k value: {}", jwks_k);
    println!(
        "JWKS decoded: {:?}",
        String::from_utf8_lossy(&base64url_decode(&jwks_k))
    );

    // Decode the k value exactly as the JWKS validation code does
    let decoded_k = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&jwks_k)
        .unwrap();
    println!("Decoded k length: {} bytes", decoded_k.len());
    println!("Decoded k bytes: {:?}", decoded_k);

    // Create the DecodingKey exactly as JWKS validation code does
    let dk = DecodingKey::from_secret(&decoded_k);

    // Create the token exactly as the test does
    let header = Header {
        kid: Some(kid.to_string()),
        alg: Algorithm::HS256,
        typ: Some("at+jwt".to_string()),
        ..Default::default()
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let claims = json!({
        "iss": "https://issuer.example",
        "aud": "my-api",
        "exp": now + 3600,
        "scope": "read write"
    });

    let token = jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret),
    )
    .unwrap();
    println!("\nGenerated token: {}", token);

    // Parse header to check typ
    let parsed_header = decode_header(&token).unwrap();
    println!("Token header typ: {:?}", parsed_header.typ);
    println!("Token header kid: {:?}", parsed_header.kid);
    println!("Token header alg: {:?}", parsed_header.alg);

    // Try verification with the DecodingKey
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.set_required_spec_claims(&["exp"]);
    validation.set_issuer(&["https://issuer.example"]);
    validation.set_audience(&["my-api"]);
    validation.leeway = 30;

    match decode::<Claims>(&token, &dk, &validation) {
        Ok(data) => {
            println!("\n=== VERIFICATION SUCCEEDED ===");
            println!("Claims: {:?}", data.claims);
        }
        Err(e) => {
            println!("\n=== VERIFICATION FAILED ===");
            println!("Error: {:?}", e);
            println!("Error kind: {:?}", e.kind());
        }
    }

    // Also try without iss/aud validation to narrow down the issue
    println!("\n=== Try without iss/aud ===");
    let mut val2 = Validation::new(Algorithm::HS256);
    val2.validate_exp = true;
    val2.set_required_spec_claims(&["exp"]);
    match decode::<Claims>(&token, &dk, &val2) {
        Ok(data) => println!("Succeeded (no iss/aud): {:?}", data.claims),
        Err(e) => println!("Failed: {:?}", e),
    }
}
