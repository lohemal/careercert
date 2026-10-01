//! 학교 로고 시험 — 시험용 로고는 코드로 그린다(실제 학교 로고를 쓰지 않는다).

use image::{ImageFormat, Rgba, RgbaImage};

use super::*;

/// 가상 컬러 로고: 파란 원 + 빨간 사각형, 바깥은 투명
pub(crate) fn fake_logo(w: u32, h: u32, format: ImageFormat) -> Vec<u8> {
    let mut img = RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 0]));
    let (cx, cy, r) = (w as f32 / 2.0, h as f32 / 2.0, w.min(h) as f32 * 0.45);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
        if d < r {
            *p = Rgba([20, 80, 200, 255]);
        }
        if (x as f32 - cx).abs() < r * 0.3 && (y as f32 - cy).abs() < r * 0.3 {
            *p = Rgba([220, 30, 30, 255]);
        }
    }
    let mut out = Vec::new();
    match format {
        ImageFormat::Jpeg => image::DynamicImage::ImageRgba8(img).to_rgb8().write_to(&mut Cursor::new(&mut out), format).unwrap(),
        _ => img.write_to(&mut Cursor::new(&mut out), format).unwrap(),
    }
    out
}

#[test]
fn png_jpg_webp_를_받는다() {
    for f in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
        let l = accept(&fake_logo(300, 200, f)).unwrap_or_else(|e| panic!("{f:?}: {e:?}"));
        assert_eq!((l.width, l.height), (300, 200), "{f:?}");
        assert_eq!(&l.png[..8], b"\x89PNG\r\n\x1a\n", "보관본은 PNG");
        assert_eq!(l.sha256.len(), 64);
    }
}

#[test]
fn 확장자가_아니라_내용으로_가린다() {
    // GIF·BMP 는 그림이지만 받지 않는다
    for f in [ImageFormat::Gif, ImageFormat::Bmp] {
        let mut img = RgbaImage::new(40, 40);
        img.fill(255);
        let mut b = Vec::new();
        let _ = image::DynamicImage::ImageRgba8(img).to_rgb8().write_to(&mut Cursor::new(&mut b), f);
        if !b.is_empty() {
            assert_eq!(accept(&b).unwrap_err().code, "LOGO_INVALID", "{f:?}");
        }
    }
    assert!(accept(b"<svg xmlns='http://www.w3.org/2000/svg'></svg>").is_err(), "SVG");
    assert!(accept(b"not an image at all").is_err());
    assert!(accept(&[]).is_err());
}

#[test]
fn 깨진_그림은_거절한다() {
    let mut b = fake_logo(200, 200, ImageFormat::Png);
    let half = b.len() / 2;
    b.truncate(half);
    assert_eq!(accept(&b).unwrap_err().code, "LOGO_INVALID");
    let mut j = fake_logo(200, 200, ImageFormat::Jpeg);
    for x in j.iter_mut().skip(200).take(400) {
        *x = 0xFF;
    }
    j.truncate(j.len() / 3);
    assert!(accept(&j).is_err());
}

#[test]
fn 너무_크거나_작으면_거절하고_큰_로고는_줄여서_보관한다() {
    assert!(accept(&fake_logo(10, 10, ImageFormat::Png)).is_err(), "너무 작음");
    assert!(accept(&fake_logo(8001, 20, ImageFormat::Png)).is_err(), "가로 8000 초과");
    let big = accept(&fake_logo(3000, 1500, ImageFormat::Png)).unwrap();
    assert_eq!((big.width, big.height), (1200, 600), "종횡비를 지키며 줄인다");
    let huge = vec![0u8; MAX_FILE_BYTES + 1];
    assert!(accept(&huge).unwrap_err().user_message.contains("10MB"));
}

#[test]
fn 증명서용_그림은_흑백이고_옅다() {
    let l = accept(&fake_logo(200, 200, ImageFormat::Png)).unwrap();
    let mark = watermark_png(&l).unwrap();
    let img = image::load_from_memory(&mark).unwrap().to_rgba8();
    let mut opaque = 0;
    for p in img.pixels() {
        let [r, g, b, a] = p.0;
        assert!(r == g && g == b, "흑백");
        if a > 0 {
            opaque += 1;
            assert!(r >= 255 - (255.0 * MARK_STRENGTH).ceil() as u8 - 1, "옅다: {r}");
        }
    }
    assert!(opaque > 0);
    assert_eq!(watermark_png(&l).unwrap(), mark, "같은 로고 → 같은 그림");
}

#[test]
fn 같은_그림이면_같은_지문() {
    let a = accept(&fake_logo(120, 80, ImageFormat::Png)).unwrap();
    let b = accept(&fake_logo(120, 80, ImageFormat::Png)).unwrap();
    let c = accept(&fake_logo(80, 120, ImageFormat::Png)).unwrap();
    assert_eq!(a.sha256, b.sha256);
    assert_ne!(a.sha256, c.sha256);
    assert_eq!(from_stored(a.png.to_vec()).unwrap().sha256, a.sha256);
}

#[test]
fn base64_는_표준과_같다() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
}
