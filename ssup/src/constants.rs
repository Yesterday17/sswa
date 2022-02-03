use lazy_static::lazy_static;
use parking_lot::RwLock;

lazy_static! {
    pub(crate) static ref USER_AGENT: RwLock<String> = RwLock::new(String::from("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/63.0.3239.108"));
}

pub fn set_useragent(user_agent: String) {
    *USER_AGENT.write() = user_agent;
}