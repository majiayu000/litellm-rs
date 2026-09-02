use crate::core::models::openai::ContentPart;

pub(super) fn collect_parts(
    message_index: usize,
    parts: &[ContentPart],
    fragments: &mut Vec<String>,
) -> super::ScanResult {
    let mut text_group = String::new();
    for (part_index, content_part) in parts.iter().enumerate() {
        if let ContentPart::Text { text } = content_part {
            if !text.is_empty() {
                if !text_group.is_empty() {
                    text_group.push('\n');
                }
                text_group.push_str(text);
            }
            continue;
        }
        super::push_fragment(fragments, &text_group);
        text_group.clear();
        super::part(message_index, part_index, content_part, fragments)?;
    }
    super::push_fragment(fragments, &text_group);
    Ok(())
}
