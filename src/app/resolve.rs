use super::*;

pub(crate) fn resolve_headers(
    headers: &[domain::Header],
    environment: &Environment,
) -> Result<Vec<domain::Header>, String> {
    headers
        .iter()
        .map(|header| {
            Ok(domain::Header {
                name: resolve_template(&header.name, environment)?,
                value: resolve_template(&header.value, environment)?,
                enabled: header.enabled,
            })
        })
        .collect()
}

pub(crate) fn resolve_auth(auth: &domain::Auth, environment: &Environment) -> Result<domain::Auth, String> {
    Ok(match auth {
        domain::Auth::None => domain::Auth::None,
        domain::Auth::Basic {
            username_ref,
            password_ref,
        } => domain::Auth::Basic {
            username_ref: resolve_template(username_ref, environment)?,
            password_ref: resolve_template(password_ref, environment)?,
        },
        domain::Auth::Bearer { token_ref } => domain::Auth::Bearer {
            token_ref: resolve_template(token_ref, environment)?,
        },
        domain::Auth::ApiKey {
            name,
            value_ref,
            location,
        } => domain::Auth::ApiKey {
            name: resolve_template(name, environment)?,
            value_ref: resolve_template(value_ref, environment)?,
            location: location.clone(),
        },
    })
}

pub(crate) fn resolve_template(value: &str, environment: &Environment) -> Result<String, String> {
    let mut resolved = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("{{") {
        let (prefix, after_start) = rest.split_at(start);
        resolved.push_str(prefix);
        let after_start = &after_start[2..];
        let Some(end) = after_start.find("}}") else {
            return Err(format!("Unclosed environment variable in `{value}`."));
        };
        let key = after_start[..end].trim();
        let Some(variable) = environment
            .variables
            .iter()
            .find(|item| item.enabled && item.name.as_ref() == key)
        else {
            return Err(format!("Missing environment variable `{key}`."));
        };
        resolved.push_str(variable.value.as_ref());
        rest = &after_start[end + 2..];
    }
    resolved.push_str(rest);
    Ok(resolved)
}

pub(crate) fn apply_auth(request: &mut domain::Request) {
    match &request.auth {
        domain::Auth::None => {}
        domain::Auth::Basic {
            username_ref,
            password_ref,
        } => request.headers.push(domain::Header::new(
            "Authorization",
            format!(
                "Basic {}",
                base64_encode(format!("{username_ref}:{password_ref}").as_bytes())
            ),
        )),
        domain::Auth::Bearer { token_ref } => request.headers.push(domain::Header::new(
            "Authorization",
            format!("Bearer {token_ref}"),
        )),
        domain::Auth::ApiKey {
            name,
            value_ref,
            location,
        } => match location {
            domain::ApiKeyLocation::Header => {
                request.headers.push(domain::Header::new(name, value_ref))
            }
            domain::ApiKeyLocation::Query => {
                request.query.push(domain::Header::new(name, value_ref))
            }
            domain::ApiKeyLocation::Cookie => {
                let cookie = format!("{name}={value_ref}");
                if let Some(existing) = request
                    .headers
                    .iter_mut()
                    .find(|header| header.name.eq_ignore_ascii_case("cookie"))
                {
                    existing.value = format!("{}; {cookie}", existing.value);
                } else {
                    request.headers.push(domain::Header::new("Cookie", cookie));
                }
            }
        },
    }
}

pub(crate) fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

pub(crate) fn response_body_for_mode(body: &str, mode: BodyViewMode) -> SharedString {
    match mode {
        BodyViewMode::Raw => body.to_string().into(),
        BodyViewMode::Pretty => serde_json::from_str::<serde_json::Value>(body)
            .and_then(|value| serde_json::to_string_pretty(&value))
            .unwrap_or_else(|_| body.to_string())
            .into(),
    }
}

pub(crate) fn format_byte_count(bytes: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

pub(crate) fn suggest_response_filename(response: &ResponseRecord) -> String {
    let extension = response
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("content-type"))
        .map(|header| extension_for_content_type(header.value.as_ref()))
        .unwrap_or("txt");
    format!("response-{}.{}", response.status, extension)
}

pub(crate) fn extension_for_content_type(content_type: &str) -> &'static str {
    let lowered = content_type.to_ascii_lowercase();
    let head = lowered.split(';').next().unwrap_or("").trim();
    match head {
        "application/json" | "application/problem+json" => "json",
        "application/xml" | "text/xml" => "xml",
        "text/html" => "html",
        "text/css" => "css",
        "text/javascript" | "application/javascript" => "js",
        "text/csv" => "csv",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "application/octet-stream" => "bin",
        s if s.starts_with("text/") => "txt",
        _ => "txt",
    }
}

pub(crate) fn format_json_body(body: &str) -> Result<String, String> {
    serde_json::from_str::<serde_json::Value>(body)
        .map_err(|error| error.to_string())
        .and_then(|value| serde_json::to_string_pretty(&value).map_err(|error| error.to_string()))
}


pub(crate) fn stable_key_hash(key: &str) -> usize {
    key.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    })
}
