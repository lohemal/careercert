//! 학교 로고 (v0.1.2) — 받아들이기·보관 형태·증명서용 연한 흑백 그림.
//!
//! * **확장자를 믿지 않는다.** 파일 머리로 형식을 가리고(PNG·JPEG·WebP 만), 실제로 끝까지 풀어 본다.
//!   깨진 파일·다른 형식(GIF·BMP·SVG …)은 거절한다.
//! * 크기 제한: 파일 10 MB, 가로·세로 각 16~8000 픽셀. 고해상도 로고(예: 4000×4000)도 받는다.
//! * 보관은 **이 프로그램이 만든 PNG 사본**(긴 변 1200 픽셀 이하로 줄임, 색은 그대로) — 원본 파일은 건드리지 않고,
//!   원본 경로도 남기지 않는다. 지문(`sha256`)은 이 보관본의 바이트로 계산한다.
//! * 증명서에는 `watermark_png` 가 만든 그림만 쓴다 — 흑백(밝기)으로 바꾸고 흰색 쪽으로 옅게 만든 것.
//!   CSS `filter`·`opacity` 에 기대지 않고 그림 자체를 만들어 넣으므로 미리보기·PDF·프린터가 같은 모습이다.

use std::io::Cursor;
use std::sync::Arc;

use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, Limits, RgbaImage};

use crate::error::{AppError, AppResult};

pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
pub const MIN_SIDE: u32 = 16;
pub const MAX_SIDE: u32 = 8000;
/// 보관본의 긴 변 (출력에는 이 정도면 충분하다 — A4 경력표 너비가 약 18cm)
pub const STORE_SIDE: u32 = 1200;
/// 증명서에 넣을 그림의 긴 변
const MARK_SIDE: u32 = 900;
/// 로고의 진하기 — 검은 부분이 흰 바탕에서 이 비율만큼만 어두워진다 (0.08~0.15 를 비교해 0.12 로 정함)
pub const MARK_STRENGTH: f32 = 0.12;

/// 보관한 로고 (PNG). Debug 는 바이트를 찍지 않는다.
#[derive(Clone)]
pub struct Logo {
    pub sha256: String,
    pub png: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

impl PartialEq for Logo {
    fn eq(&self, o: &Self) -> bool {
        self.sha256 == o.sha256
    }
}
impl Eq for Logo {}

impl std::fmt::Debug for Logo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Logo({}×{}, {})", self.width, self.height, &self.sha256[..12.min(self.sha256.len())])
    }
}

fn reject(msg: impl Into<String>) -> AppError {
    AppError::new("LOGO_INVALID", msg)
}

fn limits() -> Limits {
    let mut l = Limits::default();
    l.max_image_width = Some(MAX_SIDE);
    l.max_image_height = Some(MAX_SIDE);
    l.max_alloc = Some(512 * 1024 * 1024);
    l
}

fn decode(bytes: &[u8], allow: &[ImageFormat]) -> AppResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| reject("그림 파일을 읽지 못했습니다."))?;
    match reader.format() {
        Some(f) if allow.contains(&f) => {
            // JPEG 해독기는 끝이 잘린 파일도 남은 부분을 채워 열어 버린다 — 끝 표시(FFD9)가 있어야 온전한 파일
            if f == ImageFormat::Jpeg {
                let tail = &bytes[bytes.len().saturating_sub(64)..];
                if !tail.windows(2).any(|w| w == [0xFF, 0xD9]) {
                    return Err(reject("그림 파일이 손상되었거나 열 수 없습니다."));
                }
            }
        }
        _ => return Err(reject("PNG·JPG·WebP 그림만 등록할 수 있습니다.")),
    }
    reader.limits(limits());
    reader.decode().map_err(|e| match e {
        image::ImageError::Limits(_) => reject(format!("그림이 너무 큽니다. 가로·세로 {MAX_SIDE}픽셀 이하로 줄여 주세요.")),
        _ => reject("그림 파일이 손상되었거나 열 수 없습니다."),
    })
}

/// 사용자가 고른 파일 바이트 → 검사 → 보관용 PNG.
pub fn accept(bytes: &[u8]) -> AppResult<Logo> {
    if bytes.is_empty() {
        return Err(reject("빈 파일입니다."));
    }
    if bytes.len() > MAX_FILE_BYTES {
        return Err(reject("그림 파일이 너무 큽니다. 10MB 이하의 로고 파일을 골라 주세요."));
    }
    let img = decode(bytes, &[ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP])?;
    let (w, h) = img.dimensions();
    if w < MIN_SIDE || h < MIN_SIDE {
        return Err(reject(format!("그림이 너무 작습니다. 가로·세로 {MIN_SIDE}픽셀 이상이어야 합니다.")));
    }
    if w > MAX_SIDE || h > MAX_SIDE {
        return Err(reject(format!("그림이 너무 큽니다. 가로·세로 {MAX_SIDE}픽셀 이하로 줄여 주세요.")));
    }
    let img = if w.max(h) > STORE_SIDE { img.resize(STORE_SIDE, STORE_SIDE, image::imageops::FilterType::Lanczos3) } else { img };
    let rgba = img.to_rgba8();
    let png = encode_png(&rgba)?;
    Ok(Logo { sha256: crate::crypto::plain_sha256(&png), width: rgba.width(), height: rgba.height(), png: Arc::new(png) })
}

/// 보관본(PNG 바이트)을 다시 연다 — 발급 기록·설정에서 읽을 때. 지문이 맞는지는 부르는 쪽이 본다.
pub fn from_stored(png: Vec<u8>) -> AppResult<Logo> {
    let img = decode(&png, &[ImageFormat::Png])?;
    let (w, h) = img.dimensions();
    Ok(Logo { sha256: crate::crypto::plain_sha256(&png), width: w, height: h, png: Arc::new(png) })
}

fn encode_png(img: &RgbaImage) -> AppResult<Vec<u8>> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .map_err(|_| AppError::internal("로고 그림을 만들지 못했습니다."))?;
    Ok(out)
}

/// 증명서용 그림 — 밝기만 남기고(흑백) 흰색 쪽으로 옅게. 투명한 곳은 투명 그대로.
/// 같은 로고면 언제나 같은 바이트가 나온다(같은 입력 → 같은 계산).
pub fn watermark_png(logo: &Logo) -> AppResult<Vec<u8>> {
    let img = image::load_from_memory_with_format(&logo.png, ImageFormat::Png)
        .map_err(|_| AppError::internal("로고 그림을 열지 못했습니다."))?;
    let img = if img.width().max(img.height()) > MARK_SIDE {
        img.resize(MARK_SIDE, MARK_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() {
        let [r, g, b, a] = p.0;
        // ITU-R BT.601 밝기
        let l = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
        let v = (255.0 - (255.0 - l) * MARK_STRENGTH).round().clamp(0.0, 255.0) as u8;
        p.0 = [v, v, v, a];
    }
    encode_png(&rgba)
}

/// `data:image/png;base64,…`
pub fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64(png))
}

fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
#[path = "logo_tests.rs"]
pub(crate) mod logo_tests;
