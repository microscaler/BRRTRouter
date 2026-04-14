examples/pet_store/config/config.yaml
    # - "https://example.com"
    # - "https://api.example.com"
    # Use "*" to allow all origins (insecure, not recommended for production)
    # - "*"

Bug: Pet Store example will panic due to CORS config mismatch
The Pet Store example will panic at startup due to an incompatible configuration. The OpenAPI spec defines multiple routes with allowCredentials: true (e.g., /pets, /users, /admin/settings, /events, /secure), but the default config.yaml has the origins list commented out (empty). The RouteCorsConfig::with_origins() method explicitly panics when credentials are enabled but no origins are configured, as this violates the CORS specification. The example needs either origins configured in config.yaml or the allowCredentials settings removed from the OpenAPI spec.

Additional Locations (1)
examples/pet_store/doc/openapi.yaml#L19-L26

This must be fixed in templates/config.yaml

