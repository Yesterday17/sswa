use std::time::{Duration, SystemTime, UNIX_EPOCH};
use anyhow::bail;
use cookie::Cookie;
use md5::{Digest, Md5};
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};

/// 存储用户的登录信息
#[derive(Serialize, Deserialize, Debug)]
pub struct Credential {
    pub(crate) cookie_info: CookieInfo,
    pub(crate) sso: Vec<String>,
    pub(crate) token_info: TokenInfo,
}

impl Credential {
    pub async fn get_qrcode() -> anyhow::Result<Value> {
        let mut form = json!({
            "appkey": "4409e2ce8ffd12b8",
            "local_id": "0",
            "ts": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
        });
        let urlencoded = serde_urlencoded::to_string(&form)?;
        let sign = Credential::sign(&urlencoded, "59b43e04ad6965f34319062b478f83dd");
        form["sign"] = Value::from(sign);
        Ok(reqwest::Client::new()
            .post("http://passport.bilibili.com/x/passport-tv-login/qrcode/auth_code")
            .form(&form)
            .send()
            .await?
            .json()
            .await?)
    }

    fn sign(param: &str, app_sec: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(format!("{param}{app_sec}"));
        format!("{:x}", hasher.finalize())
    }

    pub async fn from_qrcode(value: Value) -> anyhow::Result<Self> {
        #[derive(Deserialize, Debug)]
        pub struct ResponseData {
            pub code: i32,
            pub data: ResponseValue,
            // message: String,
            // ttl: u8,
        }

        #[derive(Deserialize, Debug)]
        #[serde(untagged)]
        pub enum ResponseValue {
            Login(Credential),
            Value(serde_json::Value),
        }

        let mut form = json!({
            "appkey": "4409e2ce8ffd12b8",
            "local_id": "0",
            "auth_code": value["data"]["auth_code"],
            "ts": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
        });
        let urlencoded = serde_urlencoded::to_string(&form)?;
        let sign = Credential::sign(&urlencoded, "59b43e04ad6965f34319062b478f83dd");
        form["sign"] = Value::from(sign);
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let res: ResponseData = reqwest::Client::new()
                .post("http://passport.bilibili.com/x/passport-tv-login/qrcode/poll")
                .form(&form)
                .send()
                .await?
                .json()
                .await?;
            match res {
                ResponseData {
                    code: 0,
                    data: ResponseValue::Login(info),
                    ..
                } => {
                    return Ok(info);
                }
                ResponseData { code: 86039, .. } => {
                    // 二维码尚未确认;
                    // form["ts"] = Value::from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
                }
                _ => {
                    bail!("{:#?}", res)
                }
            }
        }
    }
}

/// 存储 Cookie 信息
#[derive(Serialize, Deserialize, Debug)]
pub struct CookieInfo {
    pub(crate) cookies: Vec<CookieEntry>,
}

/// Cookie 项
#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Deserialize, Serialize, Debug)]
pub struct TokenInfo {
    pub(crate) access_token: String,
    expires_in: u32,
    mid: u32,
    refresh_token: String,
}
