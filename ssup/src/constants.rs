use lazy_static::lazy_static;
use parking_lot::RwLock;

lazy_static! {
    pub(crate) static ref USER_AGENT: RwLock<String> = RwLock::new(String::from(
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/63.0.3239.108"
    ));
    pub(crate) static ref CONCURRENCY: RwLock<usize> = RwLock::new(3);
}

/// 设置所有请求中使用的 User-Agent
pub fn set_useragent(user_agent: String) {
    *USER_AGENT.write() = user_agent;
}

/// 设置分P上传的并发数
pub fn set_concurrency(concurrency: usize) {
    if concurrency > 0 {
        *CONCURRENCY.write() = concurrency;
    }
}
