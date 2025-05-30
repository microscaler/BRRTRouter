use crate::spec::SecurityScheme;
use std::collections::HashMap;

pub struct SecurityRequest<'a> {
    pub headers: &'a HashMap<String, String>,
    pub query: &'a HashMap<String, String>,
    pub cookies: &'a HashMap<String, String>,
}

pub trait SecurityProvider: Send + Sync {
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool;
}
