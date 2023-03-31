use bytes::Bytes;

#[derive(Debug)]
pub struct ModuleResponse<Data = Bytes> {
    pub data: Data,
}

impl<Data> ModuleResponse<Data> {
    pub fn new(data: Data) -> Self {
        Self { data }
    }
}

impl<Data> From<Data> for ModuleResponse<Data> {
    fn from(data: Data) -> Self {
        Self { data }
    }
}
