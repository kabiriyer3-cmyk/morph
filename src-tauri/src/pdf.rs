//! Minimal PDF writer: one JPEG image per page (DCTDecode, no re-compression).

pub struct PdfImage {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

fn put(out: &mut Vec<u8>, offsets: &mut [usize], id: usize, body: &[u8]) {
    offsets[id] = out.len();
    out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
    out.extend_from_slice(body);
    out.extend_from_slice(b"\nendobj\n");
}

fn put_stream(out: &mut Vec<u8>, offsets: &mut [usize], id: usize, dict: &str, data: &[u8]) {
    offsets[id] = out.len();
    out.extend_from_slice(format!("{id} 0 obj\n<< {dict} /Length {} >>\nstream\n", data.len()).as_bytes());
    out.extend_from_slice(data);
    out.extend_from_slice(b"\nendstream\nendobj\n");
}

/// `page_mm`: None = page matches the image; otherwise paper size in millimetres.
/// `orientation`: auto | portrait | landscape.
pub fn build(images: &[PdfImage], page_mm: Option<(f64, f64)>, orientation: &str, margin_mm: f64) -> Vec<u8> {
    const MM: f64 = 72.0 / 25.4;
    let n = images.len();
    let total = 3 + 3 * n;
    let mut offsets = vec![0usize; total];
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + 3 * i)).collect();
    put(&mut out, &mut offsets, 1, b"<< /Type /Catalog /Pages 2 0 R >>");
    put(
        &mut out,
        &mut offsets,
        2,
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), n).as_bytes(),
    );

    for (i, img) in images.iter().enumerate() {
        let (pid, cid, iid) = (3 + 3 * i, 4 + 3 * i, 5 + 3 * i);
        let iw = (img.width.max(1) as f64) * 0.75;
        let ih = (img.height.max(1) as f64) * 0.75;
        let (pw, ph, m) = match page_mm {
            None => (iw, ih, 0.0),
            Some((w, h)) => {
                let (a, b) = (w * MM, h * MM);
                let landscape = match orientation {
                    "landscape" => true,
                    "portrait" => false,
                    _ => img.width > img.height,
                };
                let (pw, ph) = if landscape { (a.max(b), a.min(b)) } else { (a.min(b), a.max(b)) };
                (pw, ph, margin_mm.max(0.0) * MM)
            }
        };
        let aw = (pw - 2.0 * m).max(1.0);
        let ah = (ph - 2.0 * m).max(1.0);
        let s = (aw / iw).min(ah / ih);
        let (dw, dh) = (iw * s, ih * s);
        let (x, y) = ((pw - dw) / 2.0, (ph - dh) / 2.0);
        put(
            &mut out,
            &mut offsets,
            pid,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] /Resources << /XObject << /Im0 {iid} 0 R >> >> /Contents {cid} 0 R >>"
            )
            .as_bytes(),
        );
        let content = format!("q {dw:.3} 0 0 {dh:.3} {x:.3} {y:.3} cm /Im0 Do Q");
        put_stream(&mut out, &mut offsets, cid, "", content.as_bytes());
        put_stream(
            &mut out,
            &mut offsets,
            iid,
            &format!(
                "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode",
                img.width, img.height
            ),
            &img.jpeg,
        );
    }

    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {total}\n0000000000 65535 f \n").as_bytes());
    for off in &offsets[1..] {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {total} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes());
    out
}
