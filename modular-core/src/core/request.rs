use bytes::Bytes;

pub struct ModuleRequest<Body = Bytes> {
    pub action: String,
    pub body: Body,
}

impl<Body> ModuleRequest<Body> {
    pub fn new(action: &str, body: Body) -> Self {
        Self {
            action: action.to_owned(),
            body,
        }
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub fn body(&self) -> &Body {
        &self.body
    }
}
