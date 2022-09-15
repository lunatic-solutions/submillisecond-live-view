use lunatic::process_local;
use once_cell::sync::OnceCell;
use rand::{thread_rng, Rng};

process_local! {
    static CSRF_TOKEN: OnceCell<CsrfToken> = OnceCell::new();
}

pub struct CsrfToken {
    pub masked: String,
    pub unmasked: String,
}

impl CsrfToken {
    pub fn generate() -> Self {
        let unmasked = generate_token();
        let masked = mask(&unmasked);

        CsrfToken { masked, unmasked }
    }

    pub fn get_or_init() -> &'static Self {
        CSRF_TOKEN.with(|token| token.get_or_init(Self::generate))
    }
}

/// Generates a crypto secure random key url-safe base64 encoded.
fn generate_token() -> String {
    let mut rng = thread_rng();
    let key: [u8; 18] = rng.gen();
    base64::encode_config(key, base64::URL_SAFE)
}

/// Masks a token by xor'ing with another generated token.
fn mask(token: &str) -> String {
    let mask = generate_token();
    let exored: Vec<_> = token
        .as_bytes()
        .iter()
        .zip(mask.as_bytes().iter())
        .map(|(x1, x2)| x1 & x2)
        .collect();
    let mut masked = base64::encode_config(exored, base64::URL_SAFE);
    masked.push_str(&mask);
    masked
}
