use crate::utils::error::gateway_error::GatewayError;
use bytes::Bytes;

pub(super) fn extract_text_field(
    body: &Bytes,
    content_type: &str,
    field_name: &str,
) -> Option<String> {
    let boundary = boundary(content_type)?;
    let marker = format!("--{boundary}");
    let body = String::from_utf8_lossy(body);

    for raw_part in body.split(&marker).skip(1) {
        if raw_part.starts_with("--") {
            continue;
        }
        let part = raw_part.trim_start_matches("\r\n");
        let Some((headers, value)) = part.split_once("\r\n\r\n") else {
            continue;
        };
        if !part_has_field_name(headers, field_name) {
            continue;
        }
        return Some(value.trim_end_matches("\r\n").trim().to_string());
    }

    None
}

pub(super) fn replace_text_field(
    body: &Bytes,
    content_type: &str,
    field_name: &str,
    replacement: &str,
) -> Result<Bytes, GatewayError> {
    let boundary = boundary(content_type)
        .ok_or_else(|| GatewayError::validation("Invalid multipart boundary"))?;
    let boundary_marker = format!("--{boundary}");
    let next_boundary_marker = format!("\r\n{boundary_marker}");
    let header_separator = b"\r\n\r\n";
    let bytes = body.as_ref();
    let mut boundary_offset = find_bytes(bytes, boundary_marker.as_bytes())
        .ok_or_else(|| GatewayError::validation("Invalid multipart data"))?;

    loop {
        let after_boundary = boundary_offset + boundary_marker.len();
        if bytes.get(after_boundary..after_boundary + 2) == Some(b"--") {
            break;
        }
        if bytes.get(after_boundary..after_boundary + 2) != Some(b"\r\n") {
            return Err(GatewayError::validation("Invalid multipart data"));
        }

        let headers_start = after_boundary + 2;
        let headers_end = find_bytes(&bytes[headers_start..], header_separator)
            .map(|offset| headers_start + offset)
            .ok_or_else(|| GatewayError::validation("Invalid multipart data"))?;
        let value_start = headers_end + header_separator.len();
        let value_end = find_bytes(&bytes[value_start..], next_boundary_marker.as_bytes())
            .map(|offset| value_start + offset)
            .ok_or_else(|| GatewayError::validation("Invalid multipart data"))?;
        let headers = std::str::from_utf8(&bytes[headers_start..headers_end])
            .map_err(|_| GatewayError::validation("Invalid multipart headers"))?;

        if part_has_field_name(headers, field_name) {
            let mut replaced =
                Vec::with_capacity(bytes.len() - (value_end - value_start) + replacement.len());
            replaced.extend_from_slice(&bytes[..value_start]);
            replaced.extend_from_slice(replacement.as_bytes());
            replaced.extend_from_slice(&bytes[value_end..]);
            return Ok(Bytes::from(replaced));
        }

        boundary_offset = value_end + 2;
    }

    Err(GatewayError::validation(format!(
        "multipart field '{field_name}' is required"
    )))
}

fn boundary(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|segment| {
        let segment = segment.trim();
        let raw_boundary = segment.strip_prefix("boundary=")?;
        let boundary = raw_boundary.trim_matches('"').trim();
        (!boundary.is_empty()).then(|| boundary.to_string())
    })
}

fn part_has_field_name(headers: &str, field_name: &str) -> bool {
    headers.lines().any(|line| {
        let line = line.trim();
        line.to_ascii_lowercase()
            .starts_with("content-disposition:")
            && (line.contains(&format!("name=\"{field_name}\""))
                || line.contains(&format!("name={field_name}")))
    })
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    (!needle.is_empty())
        .then(|| {
            haystack
                .windows(needle.len())
                .position(|window| window == needle)
        })
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_model_field_preserves_other_text_and_binary_parts() {
        let boundary = "alias-boundary";
        let content_type = format!("multipart/form-data; boundary={boundary}");
        let mut body = Vec::new();
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\npublic-image\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\nkeep this\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"input.png\"\r\nContent-Type: image/png\r\n\r\n"
            )
            .as_bytes(),
        );
        let binary = b"\x00png\r\nbinary\xff";
        body.extend_from_slice(binary);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let body = Bytes::from(body);

        let replaced = replace_text_field(&body, &content_type, "model", "gpt-image-1-mini")
            .expect("model field should be replaceable");

        assert_eq!(
            extract_text_field(&replaced, &content_type, "model").as_deref(),
            Some("gpt-image-1-mini")
        );
        assert_eq!(
            extract_text_field(&replaced, &content_type, "prompt").as_deref(),
            Some("keep this")
        );
        assert!(find_bytes(&replaced, binary).is_some());
        assert!(find_bytes(&replaced, b"public-image").is_none());
    }
}
