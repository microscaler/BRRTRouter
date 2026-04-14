//! RFC 7239 [`Forwarded`](https://datatracker.ietf.org/doc/html/rfc7239) parsing for CORS same-origin
//! (`host` and `proto` parameters only).

/// Split a `Forwarded` field-value on commas that are **outside** double-quoted spans (RFC 7239
/// `forwarded-element` list).
fn split_forwarded_elements(input: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_dquote = false;
    let mut escape = false;
    for (i, ch) in input.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_dquote {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_dquote = false;
            }
            continue;
        }
        if ch == '"' {
            in_dquote = true;
            continue;
        }
        if ch == ',' {
            let seg = input[start..i].trim();
            if !seg.is_empty() {
                out.push(seg);
            }
            start = i + ch.len_utf8();
        }
    }
    let seg = input[start..].trim();
    if !seg.is_empty() {
        out.push(seg);
    }
    out
}

/// Split one `forwarded-element` on semicolons outside double quotes (`parameter` list).
fn split_forwarded_parameters(element: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_dquote = false;
    let mut escape = false;
    for (i, ch) in element.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_dquote {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_dquote = false;
            }
            continue;
        }
        if ch == '"' {
            in_dquote = true;
            continue;
        }
        if ch == ';' {
            let seg = element[start..i].trim();
            if !seg.is_empty() {
                out.push(seg);
            }
            start = i + ch.len_utf8();
        }
    }
    let seg = element[start..].trim();
    if !seg.is_empty() {
        out.push(seg);
    }
    out
}

fn parse_param_assignment(param: &str) -> Option<(&str, &str)> {
    let idx = param.find('=')?;
    let name = param[..idx].trim();
    let mut value = param[idx + 1..].trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value = &value[1..value.len() - 1];
    }
    Some((name, value))
}

fn default_port_for_proto(proto: &str) -> Option<u16> {
    if proto.eq_ignore_ascii_case("https") {
        Some(443)
    } else if proto.eq_ignore_ascii_case("http") {
        Some(80)
    } else {
        None
    }
}

/// Whether `host` already includes an explicit `:port` (handles bracketed IPv6).
fn host_has_explicit_port(host: &str) -> bool {
    if host.starts_with('[') {
        host.contains("]:")
    } else {
        host
            .rfind(':')
            .is_some_and(|i| host[i + 1..].parse::<u16>().is_ok())
    }
}

/// From merged `Forwarded` header field-values, return authority `host[:port]`.
///
/// Collects the first `host` and first `proto` seen across all `forwarded-element` segments so
/// HTTP stacks that merge multiple `Forwarded` lines into one comma-separated list still work
/// (e.g. first line `host=a`, second line `proto=https` → `a:443`).
pub(crate) fn authority_from_forwarded_field_values(field_values: &[&str]) -> Option<String> {
    if field_values.is_empty() {
        return None;
    }
    let combined = field_values.join(",");
    let mut host: Option<&str> = None;
    let mut proto: Option<&str> = None;
    for element in split_forwarded_elements(&combined) {
        for p in split_forwarded_parameters(element) {
            if let Some((k, v)) = parse_param_assignment(p) {
                if k.eq_ignore_ascii_case("host") && host.is_none() && !v.is_empty() {
                    host = Some(v);
                } else if k.eq_ignore_ascii_case("proto") && proto.is_none() && !v.is_empty() {
                    proto = Some(v);
                }
            }
        }
    }
    let h = host?;
    if host_has_explicit_port(h) {
        return Some(h.to_string());
    }
    if let Some(p) = proto.and_then(default_port_for_proto) {
        return Some(format!("{h}:{p}"));
    }
    Some(h.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwarded_host_only() {
        let a = authority_from_forwarded_field_values(&["host=api.example.com"]).unwrap();
        assert_eq!(a, "api.example.com");
    }

    #[test]
    fn forwarded_proto_https_adds_443() {
        let a = authority_from_forwarded_field_values(&["proto=https;host=api.example.com"]).unwrap();
        assert_eq!(a, "api.example.com:443");
    }

    #[test]
    fn forwarded_host_has_port_skips_proto() {
        let a =
            authority_from_forwarded_field_values(&["proto=http;host=api.example.com:8443"]).unwrap();
        assert_eq!(a, "api.example.com:8443");
    }

    #[test]
    fn forwarded_comma_separated_first_hop() {
        let a = authority_from_forwarded_field_values(&[r#"for=192.0.2.1;host=edge.example"#]).unwrap();
        assert_eq!(a, "edge.example");
    }

    #[test]
    fn forwarded_quoted_for_does_not_split_on_inner_comma() {
        let v = r#"for="_something";host=ok.example;proto=https"#;
        let a = authority_from_forwarded_field_values(&[v]).unwrap();
        assert_eq!(a, "ok.example:443");
    }

    #[test]
    fn forwarded_merge_two_header_lines_host_then_proto() {
        let a = authority_from_forwarded_field_values(&["host=a.example", "proto=https"]).unwrap();
        assert_eq!(a, "a.example:443");
    }
}
