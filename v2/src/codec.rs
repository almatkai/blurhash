trait JsCos {
    fn cos_js(self) -> f64;
}
impl JsCos for f64 {
    fn cos_js(self) -> f64 {
        libm::cos(self)
    }
}
// f64 port of blurhash 2.0.5. Operation ordering mirrors its TypeScript source.
const BASE83: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";
fn base83(mut value: u32, length: usize) -> String {
    let mut bytes = vec![b'0'; length];
    for byte in bytes.iter_mut().rev() {
        *byte = BASE83[(value % 83) as usize];
        value /= 83;
    }
    String::from_utf8(bytes).expect("ASCII alphabet")
}
fn unbase83(value: &str) -> Result<u32, String> {
    value.bytes().try_fold(0u32, |v, c| {
        BASE83
            .iter()
            .position(|b| *b == c)
            .map(|n| v * 83 + n as u32)
            .ok_or_else(|| "Invalid BlurHash alphabet".into())
    })
}
fn linear(value: u8) -> f64 {
    let v = f64::from(value) / 255.;
    if v <= 0.04045 {
        v / 12.92
    } else {
        libm::pow((v + 0.055) / 1.055, 2.4)
    }
}
fn srgb(value: f64) -> u8 {
    let v = value.clamp(0., 1.);
    if v <= 0.0031308 {
        (v * 12.92 * 255. + 0.5).floor() as u8
    } else {
        ((1.055 * libm::pow(v, 1. / 2.4) - 0.055) * 255. + 0.5).floor() as u8
    }
}
fn signed_pow(value: f64, exponent: f64) -> f64 {
    // Math.sign(0) is zero, unlike f64::signum(0).
    if value == 0. {
        value
    } else {
        libm::pow(value.abs(), exponent) * value.signum()
    }
}
pub fn encode(pixels: &[u8], width: u32, height: u32) -> Result<String, String> {
    if width == 0
        || height == 0
        || width > 100
        || height > 100
        || pixels.len() != width as usize * height as usize * 4
    {
        return Err(
            "Invalid hash pixel dimensions/length (zero rounded dimensions are not clamped)".into(),
        );
    }
    let mut factors = Vec::with_capacity(12);
    for cy in 0..3 {
        for cx in 0..4 {
            let mut rgb = [0.; 3];
            let norm = if cx == 0 && cy == 0 { 1. } else { 2. };
            // Upstream traverses x outermost, y innermost; summation order matters.
            for x in 0..width {
                for y in 0..height {
                    let basis = norm
                        * (std::f64::consts::PI * f64::from(cx) * f64::from(x) / f64::from(width))
                            .cos_js()
                        * (std::f64::consts::PI * f64::from(cy) * f64::from(y) / f64::from(height))
                            .cos_js();
                    let p = ((x + y * width) * 4) as usize;
                    for c in 0..3 {
                        rgb[c] += basis * linear(pixels[p + c]);
                    }
                }
            }
            for c in &mut rgb {
                *c *= 1. / f64::from(width * height);
            }
            factors.push(rgb);
        }
    }
    // Upstream Math.max(...ac.map(v => Math.max(...v))) is NOT max(abs(v)).
    let maximum = factors[1..]
        .iter()
        .flatten()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let quant = (maximum * 166. - 0.5).floor().clamp(0., 82.) as u32;
    let actual_max = f64::from(quant + 1) / 166.;
    let dc = factors[0];
    let dc_value =
        (u32::from(srgb(dc[0])) << 16) + (u32::from(srgb(dc[1])) << 8) + u32::from(srgb(dc[2]));
    let mut hash = base83(2 * 9 + 3, 1) + &base83(quant, 1) + &base83(dc_value, 4);
    for factor in &factors[1..] {
        let q = factor.map(|v| {
            (signed_pow(v / actual_max, 0.5) * 9. + 9.5)
                .floor()
                .clamp(0., 18.) as u32
        });
        hash.push_str(&base83(q[0] * 19 * 19 + q[1] * 19 + q[2], 2));
    }
    Ok(hash)
}
pub fn decode(hash: &str, width: u32, height: u32) -> Result<Vec<u8>, String> {
    if hash.len() != 28
        || !hash.is_ascii()
        || unbase83(&hash[..1])? != 21
        || width == 0
        || height == 0
        || u64::from(width) * u64::from(height) > 640_000
    {
        return Err("Invalid preview dimensions/hash".into());
    }
    let maximum = f64::from(unbase83(&hash[1..2])? + 1) / 166.;
    let dc = unbase83(&hash[2..6])?;
    let mut colors = vec![[
        linear((dc >> 16) as u8),
        linear((dc >> 8) as u8),
        linear(dc as u8),
    ]];
    for i in 1..12 {
        let value = unbase83(&hash[4 + i * 2..6 + i * 2])?;
        colors.push([
            signed_pow((f64::from(value / (19 * 19)) - 9.) / 9., 2.) * maximum,
            signed_pow((f64::from((value / 19) % 19) - 9.) / 9., 2.) * maximum,
            signed_pow((f64::from(value % 19) - 9.) / 9., 2.) * maximum,
        ]);
    }
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for y in 0..height {
        for x in 0..width {
            let mut rgb = [0.; 3];
            for cy in 0..3 {
                for cx in 0..4 {
                    let basis = (std::f64::consts::PI * f64::from(x) * (cx as f64)
                        / f64::from(width))
                    .cos_js()
                        * (std::f64::consts::PI * f64::from(y) * (cy as f64) / f64::from(height))
                            .cos_js();
                    for (c, channel) in rgb.iter_mut().enumerate() {
                        *channel += colors[cx + cy * 4][c] * basis;
                    }
                }
            }
            let p = ((x + y * width) * 4) as usize;
            for c in 0..3 {
                pixels[p + c] = srgb(rgb[c]);
            }
            pixels[p + 3] = 255;
        }
    }
    Ok(pixels)
}
pub fn png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|_| "PNG header failed")?;
        writer
            .write_image_data(pixels)
            .map_err(|_| "PNG encoding failed")?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn genuine_ts_utility_rounding_extremes_and_half_byte_boundaries() {
        let oracle: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/oracle.json")).unwrap();
        for v in oracle["srgb"].as_array().unwrap() {
            assert_eq!(
                format!("{:016x}", v["value"].as_f64().unwrap().to_bits()),
                v["valueBits"]
            );
            assert_eq!(
                super::srgb(v["value"].as_f64().unwrap()),
                v["byte"].as_u64().unwrap() as u8,
                "{}",
                v["value"]
            );
        }
    }
}
