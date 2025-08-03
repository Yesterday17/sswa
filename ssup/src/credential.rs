use anyhow::bail;
use cookie::Cookie;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 存储用户的登录信息
#[derive(Serialize, Deserialize, Debug)]
pub struct Credential {
    #[serde(default)]
    login_time: u64,
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

    pub(crate) fn sign(param: &str, app_sec: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(format!("{param}{app_sec}"));
        format!("{:x}", hasher.finalize())
    }

    pub async fn from_qrcode(value: Value) -> anyhow::Result<Self> {
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
                    data: ResponseValue::Login(mut info),
                    ..
                } => {
                    if info.login_time == 0 {
                        info.login_time = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                    }
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

    pub async fn get_nickname(&self) -> anyhow::Result<String> {
        let response: ResponseData = reqwest::Client::new()
            .get("https://api.bilibili.com/x/web-interface/nav")
            .header("Cookie", self.cookie_info.to_string())
            .send()
            .await?
            .json()
            .await?;
        if response.code != 0 {
            bail!("{:#?}", response)
        }
        match response.data {
            ResponseValue::Value(data) => Ok(data["uname"].as_str().unwrap().to_string()),
            _ => unreachable!(),
        }
    }

    pub async fn from_cookies(cookies: &CookieInfo) -> anyhow::Result<Self> {
        let qrcode = Self::get_qrcode().await?;
        let form = json!({
            "auth_code": qrcode["data"]["auth_code"],
            "csrf": cookies.get("bili_jct").unwrap(),
            "scanning_type": 3,
        });
        let response: ResponseData = reqwest::Client::new()
            .post("https://passport.snm0516.aisee.tv/x/passport-tv-login/h5/qrcode/confirm")
            .header("Cookie", cookies.to_string())
            .form(&form)
            .send()
            .await?
            .json()
            .await?;
        if response.code != 0 {
            bail!("{:#?}", response)
        }

        Self::from_qrcode(qrcode).await
    }

    fn need_refresh(&self) -> bool {
        // Token过期前30天内重新获取
        (self.login_time + self.token_info.expires_in as u64)
            < (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 30 * 86400)
    }

    pub async fn refresh(&mut self, force: bool) -> anyhow::Result<bool> {
        if force || self.need_refresh() {
            let refreshed = Credential::from_cookies(&self.cookie_info).await?;
            self.login_time = refreshed.login_time;
            self.cookie_info = refreshed.cookie_info;
            self.token_info = refreshed.token_info;
            self.sso = refreshed.sso;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// 存储 Cookie 信息
#[derive(Serialize, Deserialize, Debug)]
pub struct CookieInfo {
    pub(crate) cookies: Vec<CookieEntry>,
}

impl CookieInfo {
    pub fn new(cookies: Vec<CookieEntry>) -> Self {
        Self { cookies }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.cookies
            .iter()
            .find(|entry| entry.name == key)
            .map(|entry| entry.value.as_str())
    }
}

impl ToString for CookieInfo {
    fn to_string(&self) -> String {
        self.cookies
            .iter()
            .map(|entry| entry.to_string())
            .collect::<Vec<String>>()
            .join("; ")
    }
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

impl FromStr for CookieEntry {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, '=');
        let name = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("CookieEntry::from_str: no name"))?;
        let value = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("CookieEntry::from_str: no value"))?;
        Ok(Self {
            name: name.to_string(),
            value: value.to_string(),
        })
    }
}

impl ToString for CookieEntry {
    fn to_string(&self) -> String {
        format!("{}={}", self.name, self.value)
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct TokenInfo {
    pub(crate) access_token: String,
    expires_in: u32,
    mid: u32,
    refresh_token: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ResponseData {
    pub(crate) code: i32,
    pub(crate) data: ResponseValue,
    pub(crate) message: String,
    ttl: i32,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub(crate) enum ResponseValue {
    Login(Credential),
    Value(serde_json::Value),
}
