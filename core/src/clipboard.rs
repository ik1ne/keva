use crate::types::value::ClipData;

pub struct Clipboard {
    // clipboard-rs context
}

impl Clipboard {
    pub fn new() -> Result<Self> {
        todo!()
    }

    pub fn read(&self) -> Result<ClipData> {
        todo!()
    }

    pub fn write(&self, data: &ClipData) -> Result<()> {
        todo!()
    }

    pub fn has_content(&self) -> bool {
        todo!()
    }

    pub fn has_plain_text(&self) -> bool {
        todo!()
    }

    pub fn has_rich_data(&self) -> bool {
        todo!()
    }
}
