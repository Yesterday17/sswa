use cookie::Cookie;
use serde::{Serialize, Deserialize};

/// 存储用户的登录信息
#[derive(Serialize, Deserialize)]
pub struct Credential {
    pub(crate) cookie_info: CookieInfo,
    pub(crate) sso: Vec<String>,
    pub(crate) token_info: TokenInfo,
}

/// 存储 Cookie 信息
#[derive(Serialize, Deserialize)]
pub struct CookieInfo {
    pub(crate) cookies: Vec<CookieEntry>,
}

/// Cookie 项
#[derive(Serialize, Deserialize)]
pub struct CookieEntry {
    name: String,
    value: String,
}

impl CookieEntry {
    pub(crate) fn to_cookie(&self) -> Cookie {
        Cookie::build(self.name.clone(), self.value.clone())
            .domain("bilibili.com")
            .finish()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TokenInfo {
    pub(crate) access_token: String,
    expires_in: u32,
    mid: u32,
    refresh_token: String,
}
