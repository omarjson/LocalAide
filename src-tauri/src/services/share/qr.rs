//! QR codes for the share links, drawn as SVG. The `qrcodegen` encoder is
//! dependency-free and runs offline, so a link never leaves this computer
//! to be rendered.

/// The QR for `text`, as a compact SVG with quiet zones. Squares merge into
/// one path so the image stays small and scales cleanly.
pub fn svg(text: &str) -> Result<String, crate::app::error::AppError> {
    let qr = qrcodegen::QrCode::encode_text(
        text,
        qrcodegen::QrCodeEcc::Medium,
    )
    .map_err(|_| {
        crate::app::error::AppError::invalid_input(
            "This link is too long to fit in a QR code.",
        )
    })?;
    let size = qr.size();
    // One-module border (the norm is four, but the SVG sits on a card with
    // its own padding).
    let border = 2;
    let total = size + border * 2;
    let mut path = String::new();
    for y in 0..size {
        for x in 0..size {
            if qr.get_module(x, y) {
                path.push_str(&format!("M{x},{y}h1v1h-1z"));
            }
        }
    }
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {total} {total}\" shape-rendering=\"crispEdges\" role=\"img\" aria-label=\"QR code for the share link\"><rect width=\"{total}\" height=\"{total}\" fill=\"#ffffff\"/><path d=\"{path}\" transform=\"translate({border},{border})\" fill=\"#14100c\"/></svg>"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_builds_for_a_share_link() {
        let svg = svg("http://127.0.0.1:8080/#k=abcdef0123456789abcdef0123456789abcdef0123456789ab").unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("aria-label"));
        assert!(svg.contains("<path d=\"M"));
    }
}
