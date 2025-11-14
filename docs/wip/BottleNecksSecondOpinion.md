Analysis of BRRTRouter Main Branch and Improving the Request Router
Current BRRTRouter Implementation and Performance Issues
OpenAPI-Driven Routing: The BRRTRouter (main branch) uses an OpenAPI 3.1 specification as the single source of truth for routes. At startup, it parses the OpenAPI spec and extracts all the necessary route metadata – including each path, the HTTP method (verb), parameters (path/query/header), request body schema, response schema, and a handler identifier (derived from the spec’s operationId or a custom x-handler-name field)
GitHub
. This means every endpoint in the spec is mapped to an internal route representation that includes the handler name it should dispatch to. Regex-Based Path Matching: The current router builds its routing table by converting each path template into a regular expression. For example, an OpenAPI path like /pets/{id} becomes a regex pattern that can match URLs like /pets/123 and capture the {id} value. Route matching is then done by testing the incoming request path against all compiled regex patterns sequentially until a match is found
GitHub
. In other words, each incoming request may be compared against every route’s regex in the worst case. This design is straightforward, but it results in O(n) matching complexity where n is the number of routes
GitHub
. The BRRTRouter documentation notes it strives for sub-microsecond regex matches for simple paths and minimal allocations, but acknowledges the linear scan through routes
GitHub
. In practice, as the number of routes grows, this approach can become a bottleneck despite using compiled regexes. Dispatcher and Handler Execution: Once a route is matched, BRRTRouter uses a dispatcher to invoke the appropriate handler. The handler functions are registered (by name) in a dispatcher table at startup. In the current design, each handler runs in its own coroutine and receives requests via a channel
github.com
– the router sends the request data to the handler’s channel, and the handler coroutine processes it and returns a response (encapsulated in a HandlerResponse). This coroutine-based model (using the may library for stackful coroutines) avoids blocking the thread and gives a FastAPI-like synchronous coding style for handlers. However, the extra hop via channels and context switching is an additional overhead compared to a direct function call, though likely smaller than the cost of route matching and I/O parsing. Observed Performance Regression: Initially, BRRTRouter demonstrated impressive throughput (the author reported ~30k requests/sec on 4 cores with full request/response validation)
medium.com
. However, the latest main branch shows a significant performance drop. Likely reasons include:
Route Matching Overhead: As more routes and features were added, the regex-based matching may have become a limiting factor. Regex matching is relatively expensive, and checking every route’s pattern for each request scales poorly. In fact, recent profiling of BRRTRouter highlighted regex capture groups and HashMap allocations as hotspots in the routing code
github.com
. Some optimizations (like preallocating buffers for regex captures) trimmed ~5% off benchmark times
github.com
, but these minor gains can’t overcome the fundamental cost of sequential regex matching.
Additional Middleware and Validation: The main branch has introduced middleware for metrics, tracing, authentication, etc. For example, a MetricsMiddleware now tracks request counts/latency, TracingMiddleware creates trace spans, AuthMiddleware checks authentication headers, and so on
github.com
. These features add per-request work (e.g. incrementing counters, recording timestamps, parsing auth tokens). While each is individually small, together they contribute to higher latency and lower throughput. If the original benchmarks were closer to a “hello world” scenario, the current version doing full OpenAPI validation, metrics, and auth on every request will naturally be slower.
Typed Request Handling Overhead: The project has been evolving toward strong typing (e.g. generating Rust types from schemas and using TypedHandlerRequest). If the router now spends more time deserializing JSON into structs or converting types for handlers (via TryFrom<HandlerRequest> implementations, etc.), that could also regress performance relative to a simpler echo-handler setup. Every conversion and validation step (while extremely useful for correctness) does cost CPU time.
In summary, the combination of a less efficient routing algorithm (regex + linear scan) and the added processing per request (metrics, tracing, auth, type conversion) appears to have caused the noted performance regression. This is not a fundamental flaw in using OpenAPI or Rust, but rather an opportunity to optimize the router’s design to better scale with added functionality.
Identifying the Bottlenecks
To address the regression, it’s important to pinpoint the main contributors to request latency:
Path Matching Algorithm: The current O(n) regex matching is the prime suspect. Even if each regex match is optimized, scaling to many routes or high RPS (requests per second) will show increasing cost. For perspective, a benchmark comparing routing algorithms found that a regex-based router took about 420 µs to match among 130 routes, whereas a highly optimized trie-based router could do the same in about 2.4 µs
github.com
github.com
. That’s almost two orders of magnitude difference. BRRTRouter’s own goal is 1 million requests/sec per core (≈1 µs per request)
github.com
, which is very hard to achieve with a naive regex loop. This strongly suggests that the routing algorithm is the key area to improve.
Excessive Allocations or Copies: The profiling mentioned in BRRTRouter’s docs indicates that creating new HashMaps (likely for storing path parameters or query parameters) for each request was costly
github.com
. If, for example, every request triggers allocation of a fresh map or vector to hold captures, that overhead can accumulate. This kind of overhead can be reduced by pooling or reusing objects, or by using more efficient data structures (like fixed-size arrays for a known small number of path params, etc.).
Middleware Serial Execution: The current design runs all middlewares in sequence for every request (the dispatcher calls metrics → tracing → auth → cors, then the handler). If each middleware does some synchronous work, the latency adds up. There might not be an easy way around this (each middleware has a job to do), but ensuring each is minimal and perhaps running some checks in parallel (if possible) could help. However, routing is still the first and most fundamental step to optimize before looking at micro-optimizations in middleware.
In practice, improving the route matching algorithm will likely yield the biggest win. The rest of the processing (JSON parsing, auth checks, etc.) tends to be necessary work, whereas spending, say, 0.4 ms scanning regexes for a route that could be found in 0.004 ms is a clear inefficiency.
Designing a High-Performance Router from Scratch
To overcome these issues, we can redesign the router to use more efficient data structures and algorithms. The key ideas are:
1. Parse OpenAPI Spec at Startup into a Routing Table
   Leverage the OpenAPI spec to build a comprehensive routing table once at startup. This is already partially done in BRRTRouter, but a fresh implementation can streamline it:
   Use an OpenAPI parsing library (or the existing oas3::Spec in BRRTRouter) to read the spec (YAML or JSON). Extract all the paths and their operations. For each operation, retrieve:
   The HTTP method (GET, POST, etc.).
   The path template (e.g. /users/{id}).
   The operation’s parameters (with their names, types, and locations).
   The request body schema (if any, for POST/PUT/PATCH).
   The response schema or status codes (if needed for response validation).
   The handler name – this could be taken from operationId or a custom extension like x-handler-name as mentioned. BRRTRouter’s spec module, for instance, notes that it captures the handler name from the operation’s operationId or x-handler-name field
   GitHub
   . This string will serve as the link to your actual Rust handler function.
   Organize this extracted data into a structure, for example a list of RouteMeta objects or a nested map. The goal is to have a readily queryable representation of “given method M and path P -> here’s the handler and details”.
2. Build an Efficient Routing Structure (HashMap/Trie)
   Rather than compiling each path to a regex and doing linear scans, construct a lookup structure that can match paths quickly:
   HashMap approach: You could use a hash map keyed by the exact route strings, but because of dynamic segments ({id} etc.), you can’t directly key by the literal request path (since "users/123" should match the template "users/{id}"). However, you can break the path into segments by / and handle static vs dynamic segments:
   One simple design: a two-level map, e.g. routes[method][path_template] = RouteInfo. You could normalize path templates by replacing {param} with a placeholder (like {:} or similar) and use that as a key. For instance, store an entry for ("GET", "/users/{id}"). To match an incoming "GET /users/123" request, you could construct the template key by checking that /users/123 fits the pattern /users/{something}. This still requires some string processing, but you could do it segment by segment (check first segment "users" matches, second segment is a single parameter token, etc.). A hash map lookup on the full path template would be O(1) for exact matches, but you need a strategy to identify the correct template for a given concrete path.
   A better variant is a nested map by segments: e.g., a map for first segment, which points to either a subtree or a terminal. The first segment of /users/123 is "users" – that could be a key in a map for “/users/*”. Under that, you know the second segment is a variable, so you match it as a wildcard. This starts to resemble a tree/trie structure rather than a flat hash map.
   Trie (prefix tree) approach: This is the approach used by many high-performance routers (such as matchit, path-tree, or the Go httprouter algorithm). You create a tree where each node represents a path segment. For example, consider routes /pets/{id} and /pets/search. The trie might have a root node, a child for "pets", and beneath that child two children: one representing the literal "search" segment, and another representing a wildcard (for {id}). When a request path comes in, you split it by / and walk the trie: e.g. for /pets/123, go to root -> "pets" node -> wildcard child (since there’s no literal "123" child, take the {id} wildcard) and that yields a match with id=123. This lookup is very fast because it does a small number of pointer/map lookups and comparisons per segment, rather than testing full regexes. In the matchit crate’s radix tree, for instance, most route matches only traverse a few nodes and do very few comparisons, leading to microsecond-level match times even with large route sets
   github.com
   github.com
   .
   Complex paths: The router should also handle scenarios like overlapping routes or wildcard segments:
   Static segments have priority over dynamic ones at the same level (so if /users/new and /users/{id} both exist, a request for /users/new ideally should match the static route, not treat "new" as an {id}). A trie can enforce this naturally by checking static children before wildcard children.
   Wildcard segments (e.g. {*path} that match multiple segments) need special handling at the end of a route definition, but these are less common and can be integrated as a catch-all leaf in the trie.
   Bottom line: Using a trie or structured hash map will change the route matching from “try all patterns until one matches” to “navigate directly to the matching route”. The complexity drops from O(n) to roughly O(m) or O(m log k), where m is the number of segments in the path and k is branching factor. In practice this is a huge win. The referenced benchmark showed regex scanning was ~420 µs vs. trie ~2.4 µs for 130 routes
   github.com
   github.com
   . That kind of improvement will more than compensate for any added overhead from new features, and will set the stage for the router to meet its performance targets.
3. Minimize Per-Request Overhead
   With the routing structure optimized, ensure that other per-request operations are also efficient:
   Parameter extraction: Instead of using regex capture groups to extract path params, you can do this manually once you know which route matched. For example, if the route template is /pets/{id} and you got a match, you know the position of {id} (second segment). You can slice the request path to get that segment ("123" in /pets/123) and parse it according to the expected type (e.g. to an integer if the spec says id is an integer). This avoids regex overhead and allows using simple Rust parsing (str::parse::<u32> etc.). The router can store type info for each param from the spec, so it knows how to parse and even validate (e.g. if the spec has a minimum value, etc.). By doing this in code, you also avoid allocating new strings for each param in many cases – you can borrow substrings or use indexing on the original path string slice.
   Query parameters and headers: Similar strategy – the spec tells you which query params to expect. You can parse the query string once (split by & into a small map or struct) and then pick out the ones you need. If using serde or a library is too slow for this, a manual parse might be faster. However, since query parsing is typically done anyway, you might use a crate or even BRRTRouter’s existing query extraction logic. Just ensure it doesn’t create unnecessary intermediate allocations. Caching some of this or reusing buffers (if you control the server loop) could also help.
   Memory reuse: Consider reusing structures where possible. For example, keep a pool of RouteMatch or RequestContext objects that can be reused for each request to avoid constantly allocating new ones. This is a more advanced optimization and might be premature, but it’s something to note if aiming for extreme performance. Given Rust’s ownership, you might also achieve this by having the router functions operate on stack-allocated locals rather than heap allocations for each match.
4. Dispatch to Handlers by Name or Direct Reference
   When the router finds a matching route, it needs to call the appropriate handler. Since the spec provided a handler identifier (and your routing table stored it), you have a couple of options:
   Dynamic dispatch via a lookup: Maintain a global or easily accessible HashMap<String, HandlerFunc> where HandlerFunc is a function pointer, trait object, or enum that can be invoked for the request. For example, BRRTRouter’s Dispatcher does something akin to this – you register handlers by name upfront, e.g. dispatcher.register_handler("get_user", get_user_handler) etc., and it stores them internally
   github.com
   . When a request comes for "get_user", it uses the stored function (actually it posts the request to that handler’s coroutine). In a simpler synchronous design, you could directly call the function. A hash map lookup by handler name is very fast (amortized O(1)), especially since the number of handlers equals the number of routes typically (which is not extremely large). If you wanted to avoid even that lookup per request, you could store a direct function pointer or index in the route metadata. For instance, your RouteMeta could have handler_fn: fn(Request) -> Response set at startup. Then routing returns a reference to that function, and you just call it.
   Code generation / static dispatch: Another approach is to generate code that matches on an enum of routes or uses pattern matching to call handlers. This is more complex and usually what frameworks do behind the scenes (or via macros). Since BRRTRouter is OpenAPI-first, dynamic dispatch is fine. But if ultimate performance is desired and you have compile-time knowledge of routes (in a code-first scenario), a giant match statement or static dispatch could eliminate the hash map lookup cost. This might be over-optimization for most cases, though.
   Given your scenario (“build from scratch”), an easy and flexible method is: use the operationId as the key to bind things. For example, in the spec if you have:
   paths:
   /users/{id}:
   get:
   operationId: get_user
   parameters:
   - name: id
   in: path
   schema: { type: integer }
   responses: {...}
   You parse this, get handler_name = "get_user". In Rust, you have a function like:
   fn get_user(request: RequestContext) -> Response {
   // ...
   }
   At startup, after parsing the spec, you do something like:
   dispatcher.insert("get_user", get_user);
   or directly routes_map["GET"]["/users/{id}"].handler_fn = get_user;. Then at runtime, once the router matches a route, it can call get_user with the already-extracted id parameter and request data. This is essentially how BRRTRouter works with its dispatcher and the echo_handler in the example (where they bind "post_item" to an echo_handler function that simply mirrors back the input)
   github.com
   .
5. Retain Full Validation and Features
   Importantly, this new design should not sacrifice the features that may have caused the regression. We still want full request validation, metrics, etc., but with a more efficient core those become affordable:
   You can still perform JSON schema validation on the request or response if required, but possibly do it selectively or in a separate thread if it’s heavy. (Currently, schema validation of bodies isn’t implemented in BRRTRouter
   github.com
   – when added, it will definitely slow things down, so one might choose to only do partial validation or rely on Rust’s type system after deserialization for most checks.)
   Continue to use the middleware chain (metrics, tracing, auth). With faster routing, the relative overhead of these is higher, but they are doing necessary work. Focus on making each middleware as efficient as possible (e.g., the metrics middleware could use a static atomic counter or a lightweight histogram, etc.). Given the target is high throughput, things like avoiding locks or blocking calls in middleware is important.
   Concurrency: The coroutine model can handle a high number of concurrent requests, but ensure that the router’s data structures (like the new routing table or handler map) are either immutable or thread-safe once built. Ideally, build the routing table once (immutable thereafter), so lookups can be done without locks in a multithreaded context. Reading from a HashMap or walking a trie that’s not being modified is thread-safe in Rust (as long as you hand out only references). This way, multiple worker threads (or coroutine schedulers) can route requests in parallel with no contention.
   Providing Handler Identifiers in the OpenAPI Spec
   You specifically mentioned “the router will bind the handler name that it should route the request to in an attribute in the path, so we need to figure out how to provide said param.” In the OpenAPI spec, you have two main ways to indicate the handler name:
   Use operationId: This is a standard field in OpenAPI for exactly this purpose – a unique identifier for the operation. BRRTRouter uses operationId if present as the handler key
   GitHub
   . You should ensure each operationId in your spec is unique (OpenAPI requires uniqueness per API). For example, you might have operationId: "create_item" for a POST, operationId: "get_item" for a GET, etc. These strings can directly correspond to your Rust function names or some naming convention of your choosing.
   Use a custom extension (e.g. x-handler-name): If you prefer not to or cannot use operationId (for instance, if operationId is used for something else or you want to allow multiple mappings), you can add a custom field. OpenAPI allows arbitrary x-... fields. BRRTRouter’s parser already looks for x-handler-name as well
   GitHub
   . You would include it like:
   /items/{id}:
   get:
   summary: Get an item
   x-handler-name: get_item_handler
   responses: { ... }
   Then your router loader will read that. In code, you’d map "get_item_handler" to the actual function. (If you use both operationId and x-handler-name, you could decide one overrides the other or use operationId as a default and x-handler for special cases.)
   When building the router, make sure to extract this handler identifier and store it in your route info. That way, after matching a route, you know exactly which handler to call. This design decouples the path/method matching from the actual function call, which is great for maintainability. You can change a path in the spec without renaming the function, as long as the operationId/x-handler-name stays the same, for example. Finally, register the handler functions in your router. If using a dispatcher-like pattern, this means populating the map of name→function. If using a direct function pointer in RouteMeta, then ensure during initialization you assign the correct function to each route’s metadata. BRRTRouter’s README shows an example of registering handlers by name (using dispatcher.register_handler("post_item", echo_handler))
   github.com
   . In your own implementation, this could simply be a matter of doing something like:
   for route in routes {
   let name = route.handler_name;
   match name.as_str() {
   "get_user" => route.handler_fn = get_user,
   "post_item" => route.handler_fn = post_item,
   // ... and so on for each handler ...
   _ => return Err(format!("No handler function for name {}", name)),
   }
   }
   You might generate that match via a macro or just write it manually for a small number of handlers. Alternatively, use a HashMap: handlers.insert("get_user", get_user); and at runtime do handlers[&route.handler_name](request) to invoke it.
   Conclusion
   By analyzing the main branch, we determined that the significant performance regression likely stems from the routing algorithm and added per-request work. The solution is to rebuild the request router with a focus on efficient lookup and minimal overhead:
   Startup: parse the OpenAPI spec and build a structured route table (including handler mappings).
   Routing: use a direct hash map or trie-based matching instead of iterating over regexes. This will yield faster route resolution (no more O(n) scans) and thus higher throughput and lower latency per request.
   Handler Binding: include the handler identifier in the spec (via operationId or x-handler-name) and use it to connect to actual Rust functions. This provides a clear contract between the API definition and the code. The router doesn’t need to “guess” the handler; it’s explicitly provided, which you’ve noted as a requirement.
   Validation & Middleware: continue to support parameters extraction, JSON body parsing, and middleware like auth and metrics, but with the core routing optimized, the relative cost of these will be manageable. Essentially, we remove the routing as a bottleneck, so the only costs per request are those that genuinely accomplish work (auth check, DB query in handler, etc.), not the overhead of finding the handler.
   This approach aligns with the project’s vision of being “OpenAPI-native” and high-performance. The OpenAPI spec drives the server’s behavior, and we achieve performance by using algorithms appropriate for high-throughput servers (much like how well-known frameworks and API gateways do). With these changes, you should be able to regain the lost performance and even scale beyond it, approaching the design goal of millions of requests per second on suitable hardware
   GitHub
   github.com
   , all while providing automatic request parsing and validation based on the OpenAPI specification. Sources:
   BRRTRouter documentation and code comments on routing design
   GitHub
   GitHub
   and route metadata extraction
   GitHub
   .
   BRRTRouter README examples of handler registration and middleware
   github.com
   github.com
   .
   Performance analysis from BRRTRouter and external benchmarks (regex vs trie router performance)
   github.com
   github.com
   github.com
   .
   Photon framework ADR outlining the integration of BRRTRouter and its goals
   GitHub
   GitHub
   .
   Citations
   GitHub
   mod.rs

https://github.com/microscaler/BRRTRouter/blob/ea881ddd278e4ffc3d4ceca0f29716e51e036110/src/spec/mod.rs#L51-L59
GitHub
mod.rs

https://github.com/microscaler/BRRTRouter/blob/ea881ddd278e4ffc3d4ceca0f29716e51e036110/src/router/mod.rs#L19-L27
GitHub
mod.rs

https://github.com/microscaler/BRRTRouter/blob/ea881ddd278e4ffc3d4ceca0f29716e51e036110/src/router/mod.rs#L48-L54

GitHub - microscaler/BRRTRouter: BRRTRouter is a high-performance, coroutine-powered request router for Rust, driven entirely by an OpenAPI 3.1.0 Specification

https://github.com/microscaler/BRRTRouter

300k requests per second is meaningless on a hello world! I am building a new alternative and is based on an OpenAPI based router design with full request and response validation in all calls… - Charles Sibbald - Medium

https://medium.com/@casibbald/300k-requests-per-second-is-meaningless-on-a-hello-world-b8d91e321aa3

GitHub - microscaler/BRRTRouter: BRRTRouter is a high-performance, coroutine-powered request router for Rust, driven entirely by an OpenAPI 3.1.0 Specification

https://github.com/microscaler/BRRTRouter

GitHub - microscaler/BRRTRouter: BRRTRouter is a high-performance, coroutine-powered request router for Rust, driven entirely by an OpenAPI 3.1.0 Specification

https://github.com/microscaler/BRRTRouter

GitHub - ibraheemdev/matchit: A high performance, zero-copy URL router.

https://github.com/ibraheemdev/matchit

GitHub - ibraheemdev/matchit: A high performance, zero-copy URL router.

https://github.com/ibraheemdev/matchit

GitHub - microscaler/BRRTRouter: BRRTRouter is a high-performance, coroutine-powered request router for Rust, driven entirely by an OpenAPI 3.1.0 Specification

https://github.com/microscaler/BRRTRouter
GitHub
mod.rs

https://github.com/microscaler/BRRTRouter/blob/ea881ddd278e4ffc3d4ceca0f29716e51e036110/src/spec/mod.rs#L52-L59

GitHub - ibraheemdev/matchit: A high performance, zero-copy URL router.

https://github.com/ibraheemdev/matchit

GitHub - microscaler/BRRTRouter: BRRTRouter is a high-performance, coroutine-powered request router for Rust, driven entirely by an OpenAPI 3.1.0 Specification

https://github.com/microscaler/BRRTRouter
GitHub
mod.rs

https://github.com/microscaler/BRRTRouter/blob/ea881ddd278e4ffc3d4ceca0f29716e51e036110/src/spec/mod.rs#L52-L55
GitHub
001_Concept.md

https://github.com/microscaler/photon/blob/2902fea349c807410064f67a7560dbb9dc6d1c4b/docs/ADRS/001_Concept.md#L60-L67
GitHub
001_Concept.md

https://github.com/microscaler/photon/blob/2902fea349c807410064f67a7560dbb9dc6d1c4b/docs/ADRS/001_Concept.md#L40-L48
All Sources